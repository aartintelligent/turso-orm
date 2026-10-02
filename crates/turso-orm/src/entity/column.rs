//! Columns, modeled by [`ColumnTrait`] and [`ColumnDef`].
//!
//! [`ColumnDef`] is the static description the derive macro produces for
//! each field and that [`Schema`](super::Schema) consumes to emit DDL.
//! [`ColumnTrait`] is implemented on the generated `Column` enum and carries
//! the condition builders (`Column::Name.eq("x")`,
//! `Column::Age.gt(18)`).
//!
//! Two decisions shape the builders. First, every builder emits a qualified
//! `table.column` reference through [`ColumnTrait::as_column_ref`], so a
//! condition on `user.id` never clashes with `post.id` once the query joins
//! both tables. Second, the generated `Column` enum deliberately does not
//! derive `PartialEq`: with it, `Column::X.eq(v)` would resolve to
//! `PartialEq::eq` and silently produce a `bool` instead of an expression.

use turso_sql::{ColumnRef, ColumnType, Expr, Value};

use super::iden::{IdenStatic, Iterable};

/// The static description of a column, used to generate schema.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnDef {
    /// The column type.
    pub ty: ColumnType,
    /// Whether the column accepts `NULL`.
    pub nullable: bool,
    /// Whether the column carries a `UNIQUE` constraint.
    pub unique: bool,
    /// Whether a secondary index is generated for the column.
    pub indexed: bool,
    /// The default value expression, inlined as a literal in DDL.
    pub default: Option<Expr>,
}

impl ColumnDef {
    /// A non-null column of the given type with no constraints.
    pub const fn new(ty: ColumnType) -> Self {
        Self {
            ty,
            nullable: false,
            unique: false,
            indexed: false,
            default: None,
        }
    }

    /// Sets whether the column accepts `NULL`.
    #[must_use]
    pub fn nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    /// Marks the column `UNIQUE`.
    #[must_use]
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Marks the column as needing a secondary index.
    #[must_use]
    pub fn indexed(mut self) -> Self {
        self.indexed = true;
        self
    }

    /// Sets the default value expression.
    #[must_use]
    pub fn default(mut self, value: impl Into<Expr>) -> Self {
        self.default = Some(value.into());
        self
    }
}

/// A column of an entity, derived on the `Column` enum.
///
/// Provides the condition builders: `Column::Name.eq("x")`,
/// `Column::Age.gt(18)` and so on. Every builder renders the column as
/// `table.column`, so conditions stay unambiguous after a join.
pub trait ColumnTrait: IdenStatic + Iterable {
    /// The table this column belongs to.
    const TABLE: &'static str;

    /// The static definition used for schema generation.
    fn def(&self) -> ColumnDef;

    /// The column as a qualified `table.column` reference.
    fn as_column_ref(&self) -> ColumnRef {
        ColumnRef::TableColumn(Self::TABLE.into(), self.as_str().into())
    }

    /// The column as an expression.
    fn into_expr(self) -> Expr {
        Expr::Column(self.as_column_ref())
    }

    /// Builds `col = v`.
    fn eq<V: Into<Value>>(&self, v: V) -> Expr {
        self.into_expr().eq(Expr::val(v))
    }

    /// Builds `col <> v`.
    fn ne<V: Into<Value>>(&self, v: V) -> Expr {
        self.into_expr().ne(Expr::val(v))
    }

    /// Builds `col > v`.
    fn gt<V: Into<Value>>(&self, v: V) -> Expr {
        self.into_expr().gt(Expr::val(v))
    }

    /// Builds `col >= v`.
    fn gte<V: Into<Value>>(&self, v: V) -> Expr {
        self.into_expr().gte(Expr::val(v))
    }

    /// Builds `col < v`.
    fn lt<V: Into<Value>>(&self, v: V) -> Expr {
        self.into_expr().lt(Expr::val(v))
    }

    /// Builds `col <= v`.
    fn lte<V: Into<Value>>(&self, v: V) -> Expr {
        self.into_expr().lte(Expr::val(v))
    }

    /// Builds `col BETWEEN a AND b`.
    fn between<V: Into<Value>>(&self, a: V, b: V) -> Expr {
        self.into_expr().between(Expr::val(a), Expr::val(b))
    }

    /// Builds `col NOT BETWEEN a AND b`.
    fn not_between<V: Into<Value>>(&self, a: V, b: V) -> Expr {
        self.into_expr().not_between(Expr::val(a), Expr::val(b))
    }

    /// Builds `col LIKE pattern`.
    fn like(&self, pattern: &str) -> Expr {
        self.into_expr().like(pattern)
    }

    /// Builds `col NOT LIKE pattern`.
    fn not_like(&self, pattern: &str) -> Expr {
        self.into_expr().not_like(pattern)
    }

    /// Builds `col LIKE 's%'`.
    fn starts_with(&self, s: &str) -> Expr {
        self.into_expr().starts_with(s)
    }

    /// Builds `col LIKE '%s'`.
    fn ends_with(&self, s: &str) -> Expr {
        self.into_expr().ends_with(s)
    }

    /// Builds `col LIKE '%s%'`.
    fn contains(&self, s: &str) -> Expr {
        self.into_expr().contains(s)
    }

    /// Builds `col IS NULL`.
    fn is_null(&self) -> Expr {
        self.into_expr().is_null()
    }

    /// Builds `col IS NOT NULL`.
    fn is_not_null(&self) -> Expr {
        self.into_expr().is_not_null()
    }

    /// Builds `col IN (values)`.
    fn is_in<V: Into<Value>, I: IntoIterator<Item = V>>(&self, values: I) -> Expr {
        self.into_expr().is_in(values)
    }

    /// Builds `col NOT IN (values)`.
    fn is_not_in<V: Into<Value>, I: IntoIterator<Item = V>>(&self, values: I) -> Expr {
        self.into_expr().is_not_in(values)
    }

    /// Builds `col IN (subquery)`.
    fn in_subquery(&self, select: turso_sql::Select) -> Expr {
        self.into_expr().in_subquery(select)
    }

    /// Builds `col NOT IN (subquery)`.
    fn not_in_subquery(&self, select: turso_sql::Select) -> Expr {
        self.into_expr().not_in_subquery(select)
    }

    /// Builds `col = other.col`, comparing two columns rather than a column
    /// and a value, for example across a join.
    fn eq_col<C: ColumnTrait>(&self, other: C) -> Expr {
        self.into_expr().eq(other.into_expr())
    }

    /// Builds `col MATCH query` for full-text search.
    fn matches(&self, query: &str) -> Expr {
        self.into_expr().matches(query)
    }

    /// Builds `IFNULL(col, v)`.
    fn if_null<V: Into<Value>>(&self, v: V) -> Expr {
        turso_sql::Func::if_null(self.into_expr(), Expr::val(v))
    }

    /// Builds `MAX(col)`.
    fn max(&self) -> Expr {
        turso_sql::Func::max(self.into_expr())
    }

    /// Builds `MIN(col)`.
    fn min(&self) -> Expr {
        turso_sql::Func::min(self.into_expr())
    }

    /// Builds `SUM(col)`.
    fn sum(&self) -> Expr {
        turso_sql::Func::sum(self.into_expr())
    }

    /// Builds `COUNT(col)`.
    fn count(&self) -> Expr {
        turso_sql::Func::count(self.into_expr())
    }

    /// Builds `AVG(col)`.
    fn avg(&self) -> Expr {
        turso_sql::Func::avg(self.into_expr())
    }
}
