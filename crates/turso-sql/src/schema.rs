//! DDL statement builders for tables and indexes.
//!
//! SQLite has only four storage classes, so a schema cannot express "date"
//! or "boolean" natively. This module owns the logical [`ColumnType`] that
//! bridges that gap: it decides the declared type written in DDL — which is
//! what `STRICT` tables enforce — and tells `turso-orm` how to decode the
//! column back. Everything else here is a thin builder over the SQLite DDL
//! grammar, including the Turso extensions (`ALTER COLUMN`, `USING fts`,
//! `WITHOUT ROWID`) that are opt-in on the engine side.
//!
//! The module owns the DDL AST only. Rendering lives in `crate::writer`,
//! which also inlines column defaults as literals because DDL cannot carry
//! bound parameters.
//!
//! - [`Table`]: the entry point to `CREATE`, `ALTER` and `DROP TABLE`;
//! - [`ColumnDef`], [`ColumnType`], [`TableConstraint`], [`ForeignKey`]:
//!   the pieces of a table definition;
//! - [`CreateTable`], [`AlterTable`], [`DropTable`], [`CreateIndex`],
//!   [`DropIndex`]: the statements.

use crate::expr::Expr;
use crate::iden::{Ident, IntoIden};

/// The entry point to the DDL builders.
#[derive(Debug)]
pub struct Table;

impl Table {
    /// Starts a `CREATE TABLE`.
    pub fn create() -> CreateTable {
        CreateTable::default()
    }

    /// Starts an `ALTER TABLE`.
    pub fn alter() -> AlterTable {
        AlterTable::default()
    }

    /// Starts a `DROP TABLE`.
    pub fn drop() -> DropTable {
        DropTable::default()
    }
}

/// The logical column types.
///
/// SQLite has only four storage classes; the logical type decides the
/// declared type written in DDL (which also drives `STRICT` tables) and how
/// `turso-orm` decodes values. Every type maps to one of `INTEGER`, `REAL`,
/// `TEXT` and `BLOB`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColumnType {
    /// An `INTEGER`.
    Integer,
    /// An `INTEGER` holding `0` or `1`.
    Boolean,
    /// A `REAL`.
    Real,
    /// A `TEXT`.
    Text,
    /// A `TEXT` with an advisory maximum length that the engine does not
    /// enforce.
    String(Option<u32>),
    /// A `BLOB`.
    Blob,
    /// A `TEXT` in `YYYY-MM-DD` form.
    Date,
    /// A `TEXT` in `HH:MM:SS.fff` form.
    Time,
    /// A `TEXT` in `YYYY-MM-DD HH:MM:SS.fff` form.
    DateTime,
    /// A `TEXT` in RFC 3339 form.
    TimestampWithTimeZone,
    /// A `TEXT` holding a hyphenated UUID.
    Uuid,
    /// A `TEXT` holding a JSON document.
    Json,
    /// A `TEXT` holding an exact decimal.
    Decimal,
    /// The `ANY` type, only meaningful in `STRICT` tables.
    Any,
    /// A custom declared type written verbatim.
    Custom(String),
}

impl ColumnType {
    /// The declared type written in DDL.
    pub fn declared(&self) -> &str {
        match self {
            ColumnType::Integer | ColumnType::Boolean => "INTEGER",
            ColumnType::Real => "REAL",
            ColumnType::Text
            | ColumnType::String(_)
            | ColumnType::Date
            | ColumnType::Time
            | ColumnType::DateTime
            | ColumnType::TimestampWithTimeZone
            | ColumnType::Uuid
            | ColumnType::Json
            | ColumnType::Decimal => "TEXT",
            ColumnType::Blob => "BLOB",
            ColumnType::Any => "ANY",
            ColumnType::Custom(s) => s,
        }
    }
}

/// The referential actions of a foreign key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForeignKeyAction {
    /// `NO ACTION`.
    NoAction,
    /// `RESTRICT`.
    Restrict,
    /// `CASCADE`.
    Cascade,
    /// `SET NULL`.
    SetNull,
    /// `SET DEFAULT`.
    SetDefault,
}

impl ForeignKeyAction {
    /// The action's SQL spelling.
    pub(crate) fn sql(self) -> &'static str {
        match self {
            ForeignKeyAction::NoAction => "NO ACTION",
            ForeignKeyAction::Restrict => "RESTRICT",
            ForeignKeyAction::Cascade => "CASCADE",
            ForeignKeyAction::SetNull => "SET NULL",
            ForeignKeyAction::SetDefault => "SET DEFAULT",
        }
    }
}

/// A foreign-key constraint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignKey {
    /// The optional constraint name.
    pub name: Option<Ident>,
    /// The local columns.
    pub columns: Vec<Ident>,
    /// The referenced table.
    pub ref_table: Ident,
    /// The referenced columns.
    pub ref_columns: Vec<Ident>,
    /// The `ON DELETE` action.
    pub on_delete: Option<ForeignKeyAction>,
    /// The `ON UPDATE` action.
    pub on_update: Option<ForeignKeyAction>,
}

impl ForeignKey {
    /// Builds `FOREIGN KEY (columns) REFERENCES table (ref_columns)`.
    pub fn new<C: IntoIden, R: IntoIden>(
        columns: impl IntoIterator<Item = C>,
        ref_table: impl IntoIden,
        ref_columns: impl IntoIterator<Item = R>,
    ) -> Self {
        Self {
            name: None,
            columns: columns.into_iter().map(IntoIden::into_iden).collect(),
            ref_table: ref_table.into_iden(),
            ref_columns: ref_columns.into_iter().map(IntoIden::into_iden).collect(),
            on_delete: None,
            on_update: None,
        }
    }

    /// Sets the constraint name.
    #[must_use]
    pub fn name(mut self, name: impl IntoIden) -> Self {
        self.name = Some(name.into_iden());
        self
    }

    /// Sets `ON DELETE action`.
    #[must_use]
    pub fn on_delete(mut self, action: ForeignKeyAction) -> Self {
        self.on_delete = Some(action);
        self
    }

    /// Sets `ON UPDATE action`.
    #[must_use]
    pub fn on_update(mut self, action: ForeignKeyAction) -> Self {
        self.on_update = Some(action);
        self
    }
}

/// A column definition.
#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the flags mirror independent SQL column constraints"
)]
pub struct ColumnDef {
    /// The column name.
    pub name: Ident,
    /// The logical type.
    pub ty: ColumnType,
    /// Whether `NOT NULL` is set.
    pub not_null: bool,
    /// Whether the column is a single-column `PRIMARY KEY`.
    pub primary_key: bool,
    /// Whether `AUTOINCREMENT` is set; requires an `INTEGER PRIMARY KEY`.
    pub auto_increment: bool,
    /// Whether `UNIQUE` is set.
    pub unique: bool,
    /// The `DEFAULT expr`, inlined as a literal when rendered.
    pub default: Option<Expr>,
    /// The `CHECK (expr)` constraint.
    pub check: Option<Expr>,
}

impl ColumnDef {
    /// A nullable column of the given type.
    pub fn new(name: impl IntoIden, ty: ColumnType) -> Self {
        Self {
            name: name.into_iden(),
            ty,
            not_null: false,
            primary_key: false,
            auto_increment: false,
            unique: false,
            default: None,
            check: None,
        }
    }

    /// An `INTEGER` column.
    pub fn integer(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::Integer)
    }

    /// An `INTEGER` column holding a boolean.
    pub fn boolean(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::Boolean)
    }

    /// A `REAL` column.
    pub fn real(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::Real)
    }

    /// A `TEXT` column.
    pub fn text(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::Text)
    }

    /// A `TEXT` column with an advisory length.
    pub fn string(name: impl IntoIden, len: Option<u32>) -> Self {
        Self::new(name, ColumnType::String(len))
    }

    /// A `BLOB` column.
    pub fn blob(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::Blob)
    }

    /// A date column, stored as text.
    pub fn date(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::Date)
    }

    /// A naive timestamp column, stored as text.
    pub fn date_time(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::DateTime)
    }

    /// A zoned timestamp column, stored as RFC 3339 text.
    pub fn timestamp_with_time_zone(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::TimestampWithTimeZone)
    }

    /// A UUID column, stored as text.
    pub fn uuid(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::Uuid)
    }

    /// A JSON column, stored as text.
    pub fn json(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::Json)
    }

    /// A decimal column, stored as text.
    pub fn decimal(name: impl IntoIden) -> Self {
        Self::new(name, ColumnType::Decimal)
    }

    /// Sets `NOT NULL`.
    #[must_use]
    pub fn not_null(mut self) -> Self {
        self.not_null = true;
        self
    }

    /// Makes the column nullable, which is the default.
    #[must_use]
    pub fn null(mut self) -> Self {
        self.not_null = false;
        self
    }

    /// Sets `PRIMARY KEY`.
    #[must_use]
    pub fn primary_key(mut self) -> Self {
        self.primary_key = true;
        self
    }

    /// Sets `AUTOINCREMENT`.
    #[must_use]
    pub fn auto_increment(mut self) -> Self {
        self.auto_increment = true;
        self
    }

    /// Sets `UNIQUE`.
    #[must_use]
    pub fn unique_key(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Sets `DEFAULT expr`.
    #[must_use]
    pub fn default(mut self, value: impl Into<Expr>) -> Self {
        self.default = Some(value.into());
        self
    }

    /// Sets `CHECK (expr)`.
    #[must_use]
    pub fn check(mut self, expr: Expr) -> Self {
        self.check = Some(expr);
        self
    }
}

/// The table-level constraints.
#[derive(Clone, Debug, PartialEq)]
pub enum TableConstraint {
    /// A composite `PRIMARY KEY (a, b)`.
    PrimaryKey(Vec<Ident>),
    /// A composite `UNIQUE (a, b)`.
    Unique(Vec<Ident>),
    /// A `CHECK (expr)` constraint.
    Check(Expr),
    /// A foreign key.
    ForeignKey(ForeignKey),
}

/// A `CREATE TABLE` statement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateTable {
    /// The table name.
    pub(crate) name: Option<Ident>,
    /// Whether `IF NOT EXISTS` is set.
    pub(crate) if_not_exists: bool,
    /// The column definitions, in order.
    pub(crate) columns: Vec<ColumnDef>,
    /// The table-level constraints, rendered after the columns.
    pub(crate) constraints: Vec<TableConstraint>,
    /// Whether the table is `STRICT`.
    pub(crate) strict: bool,
    /// Whether the table is `WITHOUT ROWID`.
    pub(crate) without_rowid: bool,
}

impl CreateTable {
    /// Sets the table name.
    #[must_use]
    pub fn table(mut self, name: impl IntoIden) -> Self {
        self.name = Some(name.into_iden());
        self
    }

    /// Sets `IF NOT EXISTS`.
    #[must_use]
    pub fn if_not_exists(mut self) -> Self {
        self.if_not_exists = true;
        self
    }

    /// Adds a column.
    #[must_use]
    pub fn col(mut self, column: ColumnDef) -> Self {
        self.columns.push(column);
        self
    }

    /// Adds a composite primary key.
    #[must_use]
    pub fn primary_key<C: IntoIden>(mut self, columns: impl IntoIterator<Item = C>) -> Self {
        self.constraints.push(TableConstraint::PrimaryKey(
            columns.into_iter().map(IntoIden::into_iden).collect(),
        ));
        self
    }

    /// Adds a table-level unique constraint.
    #[must_use]
    pub fn unique<C: IntoIden>(mut self, columns: impl IntoIterator<Item = C>) -> Self {
        self.constraints.push(TableConstraint::Unique(
            columns.into_iter().map(IntoIden::into_iden).collect(),
        ));
        self
    }

    /// Adds a table-level check constraint.
    #[must_use]
    pub fn check(mut self, expr: Expr) -> Self {
        self.constraints.push(TableConstraint::Check(expr));
        self
    }

    /// Adds a foreign key.
    #[must_use]
    pub fn foreign_key(mut self, fk: ForeignKey) -> Self {
        self.constraints.push(TableConstraint::ForeignKey(fk));
        self
    }

    /// Makes the table `STRICT`, so the engine type-checks stored values
    /// against the declared types.
    #[must_use]
    pub fn strict(mut self) -> Self {
        self.strict = true;
        self
    }

    /// Makes the table `WITHOUT ROWID` — experimental in Turso.
    #[must_use]
    pub fn without_rowid(mut self) -> Self {
        self.without_rowid = true;
        self
    }

    /// The columns defined so far.
    pub fn columns(&self) -> &[ColumnDef] {
        &self.columns
    }

    /// The table name, if set.
    pub fn name(&self) -> Option<&Ident> {
        self.name.as_ref()
    }
}

/// The `ALTER TABLE` operations.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AlterOp {
    /// `ADD COLUMN`.
    AddColumn(ColumnDef),
    /// `DROP COLUMN`.
    DropColumn(Ident),
    /// `RENAME COLUMN a TO b`.
    RenameColumn(Ident, Ident),
    /// `RENAME TO`.
    RenameTo(Ident),
    /// The Turso extension that redefines a column in place.
    AlterColumn(Ident, ColumnDef),
}

/// An `ALTER TABLE` statement.
///
/// SQLite accepts one operation per statement, so the builder keeps a single
/// operation and the last setter wins.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AlterTable {
    /// The table name.
    pub(crate) name: Option<Ident>,
    /// The single operation to apply.
    pub(crate) op: Option<AlterOp>,
}

impl AlterTable {
    /// Sets the table name.
    #[must_use]
    pub fn table(mut self, name: impl IntoIden) -> Self {
        self.name = Some(name.into_iden());
        self
    }

    /// Sets the operation to `ADD COLUMN`.
    #[must_use]
    pub fn add_column(mut self, column: ColumnDef) -> Self {
        self.op = Some(AlterOp::AddColumn(column));
        self
    }

    /// Sets the operation to `DROP COLUMN`.
    #[must_use]
    pub fn drop_column(mut self, column: impl IntoIden) -> Self {
        self.op = Some(AlterOp::DropColumn(column.into_iden()));
        self
    }

    /// Sets the operation to `RENAME COLUMN a TO b`.
    #[must_use]
    pub fn rename_column(mut self, from: impl IntoIden, to: impl IntoIden) -> Self {
        self.op = Some(AlterOp::RenameColumn(from.into_iden(), to.into_iden()));
        self
    }

    /// Sets the operation to `RENAME TO`.
    #[must_use]
    pub fn rename_to(mut self, to: impl IntoIden) -> Self {
        self.op = Some(AlterOp::RenameTo(to.into_iden()));
        self
    }

    /// Sets the operation to the Turso extension `ALTER COLUMN name TO <new
    /// definition>`.
    #[must_use]
    pub fn alter_column(mut self, name: impl IntoIden, column: ColumnDef) -> Self {
        self.op = Some(AlterOp::AlterColumn(name.into_iden(), column));
        self
    }
}

/// A `DROP TABLE` statement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DropTable {
    /// The table name.
    pub(crate) name: Option<Ident>,
    /// Whether `IF EXISTS` is set.
    pub(crate) if_exists: bool,
}

impl DropTable {
    /// Sets the table name.
    #[must_use]
    pub fn table(mut self, name: impl IntoIden) -> Self {
        self.name = Some(name.into_iden());
        self
    }

    /// Sets `IF EXISTS`.
    #[must_use]
    pub fn if_exists(mut self) -> Self {
        self.if_exists = true;
        self
    }
}

/// A `CREATE INDEX` statement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateIndex {
    /// The index name.
    pub(crate) name: Option<Ident>,
    /// The indexed table.
    pub(crate) table: Option<Ident>,
    /// The indexed columns with their optional sort order.
    pub(crate) columns: Vec<(Ident, Option<crate::Order>)>,
    /// Whether the index is `UNIQUE`.
    pub(crate) unique: bool,
    /// Whether `IF NOT EXISTS` is set.
    pub(crate) if_not_exists: bool,
    /// The partial-index `WHERE` expression.
    pub(crate) r#where: Option<Expr>,
    /// The Turso `USING <method>` clause.
    pub(crate) using: Option<&'static str>,
}

impl CreateIndex {
    /// Starts a `CREATE INDEX`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the index name.
    #[must_use]
    pub fn name(mut self, name: impl IntoIden) -> Self {
        self.name = Some(name.into_iden());
        self
    }

    /// Sets the indexed table.
    #[must_use]
    pub fn table(mut self, table: impl IntoIden) -> Self {
        self.table = Some(table.into_iden());
        self
    }

    /// Adds an indexed column.
    #[must_use]
    pub fn col(mut self, column: impl IntoIden) -> Self {
        self.columns.push((column.into_iden(), None));
        self
    }

    /// Adds an indexed column with a sort order.
    #[must_use]
    pub fn col_order(mut self, column: impl IntoIden, order: crate::Order) -> Self {
        self.columns.push((column.into_iden(), Some(order)));
        self
    }

    /// Makes the index `UNIQUE`.
    #[must_use]
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Sets `IF NOT EXISTS`.
    #[must_use]
    pub fn if_not_exists(mut self) -> Self {
        self.if_not_exists = true;
        self
    }

    /// Sets the partial-index `WHERE` expression.
    #[must_use]
    pub fn and_where(mut self, expr: Expr) -> Self {
        self.r#where = Some(expr);
        self
    }

    /// Sets the Turso extension `USING fts` / `USING <method>` —
    /// experimental.
    #[must_use]
    pub fn using(mut self, method: &'static str) -> Self {
        self.using = Some(method);
        self
    }
}

/// A `DROP INDEX` statement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DropIndex {
    /// The index name.
    pub(crate) name: Option<Ident>,
    /// Whether `IF EXISTS` is set.
    pub(crate) if_exists: bool,
}

impl DropIndex {
    /// Starts a `DROP INDEX`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the index name.
    #[must_use]
    pub fn name(mut self, name: impl IntoIden) -> Self {
        self.name = Some(name.into_iden());
        self
    }

    /// Sets `IF EXISTS`.
    #[must_use]
    pub fn if_exists(mut self) -> Self {
        self.if_exists = true;
        self
    }
}
