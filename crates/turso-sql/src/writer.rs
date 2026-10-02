//! Rendering of the AST into SQL text and bound values, modeled by [`Statement`].
//!
//! The writer is the single place that knows how the Turso dialect is
//! spelled: identifier quoting, placeholder syntax, where parentheses are
//! required and which clauses can carry parameters. Keeping that knowledge
//! in one private type lets every builder stay a plain data structure and
//! makes the output deterministic — the same tree always renders the same
//! text, which is what makes the engine's prepared-statement cache useful.
//!
//! Values are bound as `?` parameters wherever SQLite allows it, including
//! `LIMIT` and `OFFSET`. The exceptions are `DEFAULT` clauses in DDL, which
//! SQLite requires to be literal, and `UPDATE ... LIMIT`, which is written
//! inline; both are rendered through [`Value::to_literal`].
//!
//! - [`Statement`]: the rendered output, SQL plus values in placeholder
//!   order;
//! - [`Build`]: implemented by every statement and expression type so that
//!   `.build()` and `.to_statement()` work uniformly.

use std::fmt::Write as _;

use crate::expr::{Condition, Expr, Func, Order};
use crate::iden::{ColumnRef, Ident, TableRef};
use crate::query::{
    ConflictAction, Delete, Insert, JoinType, Returning, Select, SelectItem, Update,
};
use crate::schema::{
    AlterOp, AlterTable, ColumnDef, CreateIndex, CreateTable, DropIndex, DropTable, TableConstraint,
};
use crate::value::Value;

/// A rendered statement: SQL with `?` placeholders and the values to bind.
#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    /// The SQL text.
    pub sql: String,
    /// The bound values, in placeholder order.
    pub values: Vec<Value>,
}

impl Statement {
    /// Wraps raw SQL without parameters.
    pub fn from_string(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            values: Vec::new(),
        }
    }

    /// Wraps raw SQL with `?` placeholders and the values that fill them.
    pub fn from_sql_and_values<V: Into<Value>>(
        sql: impl Into<String>,
        values: impl IntoIterator<Item = V>,
    ) -> Self {
        Self {
            sql: sql.into(),
            values: values.into_iter().map(Into::into).collect(),
        }
    }

    /// The SQL with literals inlined, for logs and tests only.
    ///
    /// The substitution is a plain scan for `?`, so a question mark inside a
    /// string literal of the SQL would be replaced too; that is acceptable
    /// for a debugging aid and is why this text is never executed.
    pub fn to_string_inlined(&self) -> String {
        let mut out = String::with_capacity(self.sql.len() + 16);
        let mut values = self.values.iter();
        for ch in self.sql.chars() {
            if ch == '?' {
                match values.next() {
                    Some(v) => out.push_str(&v.to_literal()),
                    None => out.push('?'),
                }
            } else {
                out.push(ch);
            }
        }
        out
    }
}

impl std::fmt::Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string_inlined())
    }
}

/// Anything that renders to a [`Statement`].
pub trait Build {
    /// Renders to the SQL text and the values to bind.
    fn build(&self) -> (String, Vec<Value>) {
        let stmt = self.to_statement();
        (stmt.sql, stmt.values)
    }

    /// Renders into a [`Statement`].
    fn to_statement(&self) -> Statement;

    /// Renders with literals inlined, for logs and tests only.
    fn to_string_inlined(&self) -> String {
        self.to_statement().to_string_inlined()
    }
}

/// The rendering state: the SQL text being built and the values bound so
/// far, in placeholder order.
#[derive(Default)]
struct Writer {
    /// The SQL text.
    sql: String,
    /// The bound values, in the order their `?` was written.
    values: Vec<Value>,
}

impl Writer {
    /// Appends raw text.
    fn push(&mut self, s: &str) {
        self.sql.push_str(s);
    }

    /// Appends a double-quoted identifier, doubling embedded quotes.
    fn ident(&mut self, ident: &Ident) {
        self.sql.push('"');
        self.sql.push_str(&ident.name().replace('"', "\"\""));
        self.sql.push('"');
    }

    /// Appends a `?` placeholder and records its value.
    fn param(&mut self, value: Value) {
        self.sql.push('?');
        self.values.push(value);
    }

    /// Appends a table reference with its alias.
    fn table_ref(&mut self, t: &TableRef) {
        self.ident(&t.name);
        if let Some(alias) = &t.alias {
            self.push(" AS ");
            self.ident(alias);
        }
    }

    /// Appends a column reference.
    fn column_ref(&mut self, c: &ColumnRef) {
        match c {
            ColumnRef::Column(name) => self.ident(name),
            ColumnRef::TableColumn(table, name) => {
                self.ident(table);
                self.push(".");
                self.ident(name);
            }
            ColumnRef::Asterisk => self.push("*"),
            ColumnRef::TableAsterisk(table) => {
                self.ident(table);
                self.push(".*");
            }
        }
    }

    /// Appends `items` separated by `, `, rendering each with `f`.
    fn list<T>(&mut self, items: &[T], mut f: impl FnMut(&mut Self, &T)) {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            f(self, item);
        }
    }

    /// Appends an expression.
    #[allow(clippy::too_many_lines, reason = "one arm per Expr variant")]
    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Column(c) => self.column_ref(c),
            Expr::Value(v) => self.param(v.clone()),
            Expr::Tuple(items) => {
                self.push("(");
                self.list(items, Self::expr);
                self.push(")");
            }
            Expr::Binary(l, op, r) => {
                self.operand(l);
                self.push(" ");
                self.push(op.sql());
                self.push(" ");
                self.operand(r);
            }
            Expr::Not(inner) => {
                self.push("NOT ");
                self.operand(inner);
            }
            Expr::Neg(inner) => {
                self.push("-");
                self.operand(inner);
            }
            Expr::IsNull(inner) => {
                self.operand(inner);
                self.push(" IS NULL");
            }
            Expr::IsNotNull(inner) => {
                self.operand(inner);
                self.push(" IS NOT NULL");
            }
            Expr::In(l, r) => {
                self.operand(l);
                self.push(" IN ");
                self.in_rhs(r);
            }
            Expr::NotIn(l, r) => {
                self.operand(l);
                self.push(" NOT IN ");
                self.in_rhs(r);
            }
            Expr::Between(x, a, b) => {
                self.operand(x);
                self.push(" BETWEEN ");
                self.operand(a);
                self.push(" AND ");
                self.operand(b);
            }
            Expr::NotBetween(x, a, b) => {
                self.operand(x);
                self.push(" NOT BETWEEN ");
                self.operand(a);
                self.push(" AND ");
                self.operand(b);
            }
            Expr::Like {
                expr,
                pattern,
                negated,
                escape,
            } => {
                self.operand(expr);
                self.push(if *negated { " NOT LIKE " } else { " LIKE " });
                self.operand(pattern);
                if let Some(c) = escape {
                    // The escape character is bound rather than inlined so
                    // that a quote never has to be doubled by hand.
                    self.push(" ESCAPE ");
                    self.param(Value::Text(c.to_string()));
                }
            }
            Expr::Func(f) => self.func(f),
            Expr::Subquery(s) => {
                self.push("(");
                self.select(s);
                self.push(")");
            }
            Expr::Exists(s) => {
                self.push("EXISTS (");
                self.select(s);
                self.push(")");
            }
            Expr::Case(whens, otherwise) => {
                self.push("CASE");
                for (cond, then) in whens {
                    self.push(" WHEN ");
                    self.expr(cond);
                    self.push(" THEN ");
                    self.expr(then);
                }
                if let Some(o) = otherwise {
                    self.push(" ELSE ");
                    self.expr(o);
                }
                self.push(" END");
            }
            Expr::Cast(inner, ty) => {
                self.push("CAST(");
                self.expr(inner);
                self.push(" AS ");
                self.push(ty);
                self.push(")");
            }
            Expr::Alias(inner, alias) => {
                self.expr(inner);
                self.push(" AS ");
                self.ident(alias);
            }
            Expr::Raw(sql, values) => {
                self.push(sql);
                self.values.extend(values.iter().cloned());
            }
            Expr::Paren(inner) => {
                self.push("(");
                self.expr(inner);
                self.push(")");
            }
        }
    }

    /// Appends a sub-expression, parenthesising compound operators.
    ///
    /// Wrapping every nested binary, `NOT` and `BETWEEN` node is more
    /// parentheses than strictly needed, but it means the writer never has
    /// to encode SQL precedence tables and the output is always unambiguous.
    fn operand(&mut self, e: &Expr) {
        match e {
            Expr::Binary(..)
            | Expr::Not(_)
            | Expr::Between(..)
            | Expr::NotBetween(..)
            | Expr::Like { .. } => {
                self.push("(");
                self.expr(e);
                self.push(")");
            }
            _ => self.expr(e),
        }
    }

    /// Appends the right-hand side of `IN`.
    ///
    /// SQLite rejects `IN ()`, so an empty tuple is rendered as `(NULL)`,
    /// which matches no row and keeps a filter built from an empty list
    /// well-formed.
    fn in_rhs(&mut self, e: &Expr) {
        match e {
            Expr::Tuple(items) if items.is_empty() => self.push("(NULL)"),
            Expr::Tuple(_) | Expr::Subquery(_) => self.expr(e),
            other => {
                self.push("(");
                self.expr(other);
                self.push(")");
            }
        }
    }

    /// Appends a function call.
    fn func(&mut self, f: &Func) {
        self.push(f.name);
        self.push("(");
        if f.distinct {
            self.push("DISTINCT ");
        }
        self.list(&f.args, Self::expr);
        self.push(")");
    }

    /// Appends `keyword` followed by the condition, or nothing when the
    /// condition is empty.
    fn condition(&mut self, keyword: &str, cond: &Condition) {
        if let Some(expr) = cond.clone().into_expr() {
            self.push(keyword);
            self.expr(&expr);
        }
    }

    /// Appends a `SELECT` statement.
    fn select(&mut self, s: &Select) {
        self.push("SELECT ");
        if s.distinct {
            self.push("DISTINCT ");
        }
        if s.items.is_empty() {
            self.push("*");
        } else {
            self.list(&s.items, |w, item| match item {
                SelectItem::Expr(e) => w.expr(e),
            });
        }
        if !s.from.is_empty() {
            self.push(" FROM ");
            self.list(&s.from, Self::table_ref);
        }
        if let Some((sub, alias)) = &s.from_subquery {
            self.push(if s.from.is_empty() { " FROM (" } else { ", (" });
            self.select(sub);
            self.push(") AS ");
            self.ident(alias);
        }
        for join in &s.joins {
            self.push(match join.kind {
                JoinType::Inner => " INNER JOIN ",
                JoinType::Left => " LEFT JOIN ",
                JoinType::Cross => " CROSS JOIN ",
            });
            self.table_ref(&join.table);
            if let Some(on) = &join.on {
                self.push(" ON ");
                self.expr(on);
            }
        }
        self.condition(" WHERE ", &s.r#where);
        if !s.group_by.is_empty() {
            self.push(" GROUP BY ");
            self.list(&s.group_by, Self::expr);
        }
        self.condition(" HAVING ", &s.having);
        if !s.order_by.is_empty() {
            self.push(" ORDER BY ");
            self.list(&s.order_by, |w, (e, o)| {
                w.expr(e);
                w.push(match o {
                    Order::Asc => " ASC",
                    Order::Desc => " DESC",
                });
            });
        }
        // LIMIT and OFFSET are bound as parameters so that paging never
        // changes the SQL text and the prepared statement stays cached. A
        // value beyond i64::MAX is clamped rather than rejected.
        if let Some(limit) = s.limit {
            self.push(" LIMIT ");
            self.param(Value::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
        }
        if let Some(offset) = s.offset {
            // SQLite only accepts OFFSET after LIMIT; `LIMIT -1` means no
            // limit.
            if s.limit.is_none() {
                self.push(" LIMIT -1");
            }
            self.push(" OFFSET ");
            self.param(Value::Integer(i64::try_from(offset).unwrap_or(i64::MAX)));
        }
    }

    /// Appends a `RETURNING` clause.
    fn returning(&mut self, r: &Returning) {
        match r {
            Returning::None => {}
            Returning::All => self.push(" RETURNING *"),
            Returning::Columns(cols) => {
                self.push(" RETURNING ");
                self.list(cols, Self::column_ref);
            }
        }
    }

    /// Appends an `INSERT` statement.
    fn insert(&mut self, s: &Insert) {
        self.push("INSERT INTO ");
        if let Some(t) = &s.table {
            self.table_ref(t);
        }
        if s.default_values {
            self.push(" DEFAULT VALUES");
        } else {
            if !s.columns.is_empty() {
                self.push(" (");
                self.list(&s.columns, Self::ident);
                self.push(")");
            }
            if let Some(select) = &s.select {
                self.push(" ");
                self.select(select);
            } else {
                self.push(" VALUES ");
                self.list(&s.rows, |w, row| {
                    w.push("(");
                    w.list(row, Self::expr);
                    w.push(")");
                });
            }
        }
        if let Some(oc) = &s.on_conflict {
            self.push(" ON CONFLICT");
            if !oc.target.is_empty() {
                self.push(" (");
                self.list(&oc.target, Self::ident);
                self.push(")");
            }
            match &oc.action {
                ConflictAction::Nothing => self.push(" DO NOTHING"),
                ConflictAction::Update(sets) => {
                    self.push(" DO UPDATE SET ");
                    self.list(sets, |w, (c, e)| {
                        w.ident(c);
                        w.push(" = ");
                        w.expr(e);
                    });
                }
            }
        }
        self.returning(&s.returning);
    }

    /// Appends an `UPDATE` statement.
    fn update(&mut self, s: &Update) {
        self.push("UPDATE ");
        if let Some(t) = &s.table {
            self.table_ref(t);
        }
        self.push(" SET ");
        self.list(&s.sets, |w, (c, e)| {
            w.ident(c);
            w.push(" = ");
            w.expr(e);
        });
        if let Some(cond) = &s.r#where {
            self.condition(" WHERE ", cond);
        }
        self.returning(&s.returning);
        if let Some(limit) = s.limit {
            let _ = write!(self.sql, " LIMIT {limit}");
        }
    }

    /// Appends a `DELETE` statement.
    fn delete(&mut self, s: &Delete) {
        self.push("DELETE FROM ");
        if let Some(t) = &s.table {
            self.table_ref(t);
        }
        if let Some(cond) = &s.r#where {
            self.condition(" WHERE ", cond);
        }
        self.returning(&s.returning);
    }

    /// Appends a column definition.
    fn column_def(&mut self, c: &ColumnDef) {
        self.ident(&c.name);
        self.push(" ");
        self.push(c.ty.declared());
        if c.primary_key {
            self.push(" PRIMARY KEY");
            if c.auto_increment {
                self.push(" AUTOINCREMENT");
            }
        }
        if c.not_null {
            self.push(" NOT NULL");
        }
        if c.unique {
            self.push(" UNIQUE");
        }
        if let Some(d) = &c.default {
            self.push(" DEFAULT ");
            self.default_expr(d);
        }
        if let Some(chk) = &c.check {
            self.push(" CHECK (");
            self.expr(chk);
            self.push(")");
        }
    }

    /// Appends a `DEFAULT` expression with its values inlined as literals.
    ///
    /// DDL cannot carry bound parameters, so this is the one place where
    /// values are written into the SQL text. Anything that is not a plain
    /// value is parenthesised, as SQLite requires for default expressions.
    fn default_expr(&mut self, e: &Expr) {
        match e {
            Expr::Value(v) => self.push(&v.to_literal()),
            Expr::Raw(sql, _) => {
                self.push("(");
                self.push(sql);
                self.push(")");
            }
            other => {
                let mut inner = Writer::default();
                inner.expr(other);
                let inlined = Statement {
                    sql: inner.sql,
                    values: inner.values,
                }
                .to_string_inlined();
                self.push("(");
                self.push(&inlined);
                self.push(")");
            }
        }
    }

    /// Appends a foreign-key constraint.
    fn foreign_key(&mut self, fk: &crate::schema::ForeignKey) {
        if let Some(name) = &fk.name {
            self.push("CONSTRAINT ");
            self.ident(name);
            self.push(" ");
        }
        self.push("FOREIGN KEY (");
        self.list(&fk.columns, Self::ident);
        self.push(") REFERENCES ");
        self.ident(&fk.ref_table);
        self.push(" (");
        self.list(&fk.ref_columns, Self::ident);
        self.push(")");
        if let Some(a) = fk.on_delete {
            self.push(" ON DELETE ");
            self.push(a.sql());
        }
        if let Some(a) = fk.on_update {
            self.push(" ON UPDATE ");
            self.push(a.sql());
        }
    }

    /// Appends a `CREATE TABLE` statement.
    fn create_table(&mut self, s: &CreateTable) {
        self.push("CREATE TABLE ");
        if s.if_not_exists {
            self.push("IF NOT EXISTS ");
        }
        if let Some(name) = &s.name {
            self.ident(name);
        }
        self.push(" (");
        self.list(&s.columns, Self::column_def);
        for c in &s.constraints {
            self.push(", ");
            match c {
                TableConstraint::PrimaryKey(cols) => {
                    self.push("PRIMARY KEY (");
                    self.list(cols, Self::ident);
                    self.push(")");
                }
                TableConstraint::Unique(cols) => {
                    self.push("UNIQUE (");
                    self.list(cols, Self::ident);
                    self.push(")");
                }
                TableConstraint::Check(e) => {
                    self.push("CHECK (");
                    self.expr(e);
                    self.push(")");
                }
                TableConstraint::ForeignKey(fk) => self.foreign_key(fk),
            }
        }
        self.push(")");
        let mut opts = Vec::new();
        if s.strict {
            opts.push("STRICT");
        }
        if s.without_rowid {
            opts.push("WITHOUT ROWID");
        }
        if !opts.is_empty() {
            self.push(" ");
            self.push(&opts.join(", "));
        }
    }

    /// Appends an `ALTER TABLE` statement.
    fn alter_table(&mut self, s: &AlterTable) {
        self.push("ALTER TABLE ");
        if let Some(name) = &s.name {
            self.ident(name);
        }
        match &s.op {
            Some(AlterOp::AddColumn(c)) => {
                self.push(" ADD COLUMN ");
                self.column_def(c);
            }
            Some(AlterOp::DropColumn(c)) => {
                self.push(" DROP COLUMN ");
                self.ident(c);
            }
            Some(AlterOp::RenameColumn(from, to)) => {
                self.push(" RENAME COLUMN ");
                self.ident(from);
                self.push(" TO ");
                self.ident(to);
            }
            Some(AlterOp::RenameTo(to)) => {
                self.push(" RENAME TO ");
                self.ident(to);
            }
            Some(AlterOp::AlterColumn(name, def)) => {
                self.push(" ALTER COLUMN ");
                self.ident(name);
                self.push(" TO ");
                self.column_def(def);
            }
            None => {}
        }
    }

    /// Appends a `DROP TABLE` statement.
    fn drop_table(&mut self, s: &DropTable) {
        self.push("DROP TABLE ");
        if s.if_exists {
            self.push("IF EXISTS ");
        }
        if let Some(name) = &s.name {
            self.ident(name);
        }
    }

    /// Appends a `CREATE INDEX` statement.
    fn create_index(&mut self, s: &CreateIndex) {
        self.push("CREATE ");
        if s.unique {
            self.push("UNIQUE ");
        }
        self.push("INDEX ");
        if s.if_not_exists {
            self.push("IF NOT EXISTS ");
        }
        if let Some(name) = &s.name {
            self.ident(name);
        }
        self.push(" ON ");
        if let Some(table) = &s.table {
            self.ident(table);
        }
        if let Some(method) = s.using {
            self.push(" USING ");
            self.push(method);
        }
        self.push(" (");
        self.list(&s.columns, |w, (c, o)| {
            w.ident(c);
            match o {
                Some(Order::Asc) => w.push(" ASC"),
                Some(Order::Desc) => w.push(" DESC"),
                None => {}
            }
        });
        self.push(")");
        if let Some(e) = &s.r#where {
            self.push(" WHERE ");
            self.expr(e);
        }
    }

    /// Appends a `DROP INDEX` statement.
    fn drop_index(&mut self, s: &DropIndex) {
        self.push("DROP INDEX ");
        if s.if_exists {
            self.push("IF EXISTS ");
        }
        if let Some(name) = &s.name {
            self.ident(name);
        }
    }

    /// Consumes the writer into the rendered statement.
    fn finish(self) -> Statement {
        Statement {
            sql: self.sql,
            values: self.values,
        }
    }
}

/// Implements [`Build`] for a statement type by delegating to the named
/// `Writer` method.
macro_rules! impl_build {
    ($($ty:ty => $method:ident),* $(,)?) => {$(
        impl Build for $ty {
            fn to_statement(&self) -> Statement {
                let mut w = Writer::default();
                w.$method(self);
                w.finish()
            }
        }
    )*};
}

impl_build!(
    Select => select,
    Insert => insert,
    Update => update,
    Delete => delete,
    CreateTable => create_table,
    AlterTable => alter_table,
    DropTable => drop_table,
    CreateIndex => create_index,
    DropIndex => drop_index,
);

impl Build for Expr {
    fn to_statement(&self) -> Statement {
        let mut w = Writer::default();
        w.expr(self);
        w.finish()
    }
}

impl Build for Statement {
    fn to_statement(&self) -> Statement {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    /// A select with aliases, a join, mixed `AND` / `OR` conditions,
    /// grouping, ordering and paging renders in clause order with every
    /// value bound.
    #[test]
    fn select_with_joins_and_conditions() {
        let (sql, values) = Query::select()
            .column(("u", "id"))
            .expr_as(Func::count(Expr::col(("p", "id"))), "posts")
            .from(TableRef::new("user").alias("u"))
            .left_join(
                TableRef::new("post").alias("p"),
                Expr::col(("p", "user_id")).eq(Expr::col(("u", "id"))),
            )
            .and_where(
                Condition::any()
                    .add(Expr::col(("u", "name")).like("a%"))
                    .add(Expr::col(("u", "id")).is_in([1, 2, 3])),
            )
            .and_where(Expr::col(("u", "deleted_at")).is_null())
            .group_by(Expr::col(("u", "id")))
            .and_having(Func::count(Expr::col(("p", "id"))).gt(0))
            .order_by(("u", "id"), Order::Desc)
            .limit(5)
            .offset(10)
            .build();
        assert_eq!(
            sql,
            "SELECT \"u\".\"id\", COUNT(\"p\".\"id\") AS \"posts\" FROM \"user\" AS \"u\" \
             LEFT JOIN \"post\" AS \"p\" ON \"p\".\"user_id\" = \"u\".\"id\" \
             WHERE ((\"u\".\"name\" LIKE ?) OR \"u\".\"id\" IN (?, ?, ?)) AND \"u\".\"deleted_at\" IS NULL \
             GROUP BY \"u\".\"id\" HAVING COUNT(\"p\".\"id\") > ? ORDER BY \"u\".\"id\" DESC LIMIT ? OFFSET ?"
        );
        assert_eq!(values.len(), 7);
    }

    /// Multi-row inserts with upsert, arithmetic updates and `NOT IN`
    /// deletes render correctly, checked through the inlined form.
    #[test]
    fn insert_update_delete() {
        let insert = Query::insert()
            .into_table("user")
            .columns(["name", "age"])
            .values([Expr::val("bob"), Expr::val(30)])
            .values([Expr::val("eve"), Expr::val(Option::<i32>::None)])
            .on_conflict(OnConflict::update_columns(["name"], ["age"]))
            .returning_all();
        assert_eq!(
            insert.to_string_inlined(),
            "INSERT INTO \"user\" (\"name\", \"age\") VALUES ('bob', 30), ('eve', NULL) \
             ON CONFLICT (\"name\") DO UPDATE SET \"age\" = \"excluded\".\"age\" RETURNING *"
        );

        let update = Query::update()
            .table("user")
            .value("age", Expr::col("age").add(1))
            .and_where(Expr::col("id").eq(7));
        assert_eq!(
            update.to_string_inlined(),
            "UPDATE \"user\" SET \"age\" = \"age\" + 1 WHERE \"id\" = 7"
        );

        let delete = Query::delete()
            .from_table("user")
            .and_where(Expr::col("id").is_not_in([1, 2]));
        assert_eq!(
            delete.to_string_inlined(),
            "DELETE FROM \"user\" WHERE \"id\" NOT IN (1, 2)"
        );
    }

    /// DDL renders column constraints in SQLite's order, inlines defaults
    /// as literals and appends table options after the column list.
    #[test]
    fn ddl() {
        let create = Table::create()
            .table("post")
            .if_not_exists()
            .col(ColumnDef::integer("id").primary_key().auto_increment())
            .col(ColumnDef::text("title").not_null().unique_key())
            .col(ColumnDef::boolean("published").not_null().default(false))
            .col(ColumnDef::integer("user_id").not_null())
            .col(ColumnDef::date_time("created_at").not_null())
            .foreign_key(
                ForeignKey::new(["user_id"], "user", ["id"]).on_delete(ForeignKeyAction::Cascade),
            )
            .strict();
        assert_eq!(
            create.to_string_inlined(),
            "CREATE TABLE IF NOT EXISTS \"post\" (\"id\" INTEGER PRIMARY KEY AUTOINCREMENT, \
             \"title\" TEXT NOT NULL UNIQUE, \"published\" INTEGER NOT NULL DEFAULT 0, \
             \"user_id\" INTEGER NOT NULL, \"created_at\" TEXT NOT NULL, \
             FOREIGN KEY (\"user_id\") REFERENCES \"user\" (\"id\") ON DELETE CASCADE) STRICT"
        );
        let index = CreateIndex::new()
            .name("idx_post_user")
            .table("post")
            .col("user_id")
            .if_not_exists();
        assert_eq!(
            index.to_string_inlined(),
            "CREATE INDEX IF NOT EXISTS \"idx_post_user\" ON \"post\" (\"user_id\")"
        );
        let alter = Table::alter()
            .table("post")
            .add_column(ColumnDef::text("slug"));
        assert_eq!(
            alter.to_string_inlined(),
            "ALTER TABLE \"post\" ADD COLUMN \"slug\" TEXT"
        );
        assert_eq!(
            Table::drop().table("post").if_exists().to_string_inlined(),
            "DROP TABLE IF EXISTS \"post\""
        );
    }

    /// An `IN` over an empty list renders as `IN (NULL)`, which is valid
    /// SQL that matches no row.
    #[test]
    fn empty_in_list_never_matches() {
        let sql = Query::select()
            .from("t")
            .and_where(Expr::col("id").is_in(Vec::<i32>::new()))
            .to_string_inlined();
        assert_eq!(sql, "SELECT * FROM \"t\" WHERE \"id\" IN (NULL)");
    }

    /// The `contains` family escapes wildcards in the fragment and declares
    /// the escape character, so `%` and `_` in user input match literally.
    #[test]
    fn contains_escapes_wildcards() {
        let stmt = Query::select()
            .from("t")
            .and_where(Expr::col("name").contains("50%_a\\b"))
            .to_statement();
        assert_eq!(
            stmt.sql,
            "SELECT * FROM \"t\" WHERE \"name\" LIKE ? ESCAPE ?"
        );
        assert_eq!(
            stmt.values,
            vec![
                Value::Text("%50\\%\\_a\\\\b%".into()),
                Value::Text("\\".into())
            ]
        );
        let plain = Query::select()
            .from("t")
            .and_where(Expr::col("name").not_like("a%"))
            .to_string_inlined();
        assert_eq!(plain, "SELECT * FROM \"t\" WHERE \"name\" NOT LIKE 'a%'");
    }
}
