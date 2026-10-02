//! DML statement builders: `SELECT`, `INSERT`, `UPDATE` and `DELETE`.
//!
//! Each builder is a plain data structure with chainable setters; nothing is
//! validated or rendered until the writer runs, so a partially built
//! statement can be stored, cloned and completed later — which is how
//! `turso-orm` assembles entity queries in several passes. The fields are
//! `pub(crate)` rather than private so the writer in `crate::writer` can
//! read them directly without a mirror of accessors.
//!
//! The module owns the shape of DML statements only. Expressions and
//! conditions come from `crate::expr`, identifiers from `crate::iden`, and
//! the SQL text is produced by `crate::writer`.
//!
//! - [`Query`]: the entry point, with one constructor per statement
//!   (`Query::select()` and friends);
//! - [`Select`], [`Insert`], [`Update`], [`Delete`]: the four statements;
//! - [`Join`], [`JoinType`], [`OnConflict`], [`Returning`], [`SelectItem`]:
//!   the clauses they are built from.

use crate::expr::{Condition, Expr, IntoCondition, Order};
use crate::iden::{ColumnRef, Ident, IntoIden, TableRef};
use crate::value::Value;

/// The entry point to the DML builders, mirroring the `Query::select()`
/// style.
#[derive(Debug)]
pub struct Query;

impl Query {
    /// Starts a `SELECT`.
    pub fn select() -> Select {
        Select::default()
    }

    /// Starts an `INSERT`.
    pub fn insert() -> Insert {
        Insert::default()
    }

    /// Starts an `UPDATE`.
    pub fn update() -> Update {
        Update::default()
    }

    /// Starts a `DELETE`.
    pub fn delete() -> Delete {
        Delete::default()
    }
}

/// An item of a select list.
#[derive(Clone, Debug, PartialEq)]
pub enum SelectItem {
    /// Any expression, possibly aliased with [`Expr::alias`].
    Expr(Expr),
}

/// The join kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinType {
    /// An `INNER JOIN`.
    Inner,
    /// A `LEFT JOIN`.
    Left,
    /// A `CROSS JOIN`.
    Cross,
}

/// A join clause.
#[derive(Clone, Debug, PartialEq)]
pub struct Join {
    /// The join kind.
    pub kind: JoinType,
    /// The joined table.
    pub table: TableRef,
    /// The `ON` condition, absent for a `CROSS JOIN`.
    pub on: Option<Expr>,
}

/// A `RETURNING` clause.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Returning {
    /// No `RETURNING` clause.
    #[default]
    None,
    /// `RETURNING *`.
    All,
    /// `RETURNING a, b`.
    Columns(Vec<ColumnRef>),
}

/// A `SELECT` statement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Select {
    /// Whether `DISTINCT` is set.
    pub(crate) distinct: bool,
    /// The select list; empty renders as `*`.
    pub(crate) items: Vec<SelectItem>,
    /// The `FROM` tables.
    pub(crate) from: Vec<TableRef>,
    /// A derived table `FROM (subquery) AS alias`, appended after `from`.
    pub(crate) from_subquery: Option<(Box<Select>, Ident)>,
    /// The join clauses, in order.
    pub(crate) joins: Vec<Join>,
    /// The `WHERE` condition; empty renders as nothing.
    pub(crate) r#where: Condition,
    /// The `GROUP BY` expressions.
    pub(crate) group_by: Vec<Expr>,
    /// The `HAVING` condition; empty renders as nothing.
    pub(crate) having: Condition,
    /// The `ORDER BY` terms.
    pub(crate) order_by: Vec<(Expr, Order)>,
    /// The `LIMIT`, bound as a parameter.
    pub(crate) limit: Option<u64>,
    /// The `OFFSET`, bound as a parameter.
    pub(crate) offset: Option<u64>,
}

impl Select {
    /// A new empty select whose conditions default to `Condition::all()`.
    pub fn new() -> Self {
        Self {
            r#where: Condition::all(),
            having: Condition::all(),
            ..Default::default()
        }
    }

    /// Sets `SELECT DISTINCT`.
    #[must_use]
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// Selects a column.
    #[must_use]
    pub fn column(mut self, column: impl Into<ColumnRef>) -> Self {
        self.items
            .push(SelectItem::Expr(Expr::Column(column.into())));
        self
    }

    /// Selects several columns.
    #[must_use]
    pub fn columns<C: Into<ColumnRef>>(mut self, columns: impl IntoIterator<Item = C>) -> Self {
        for c in columns {
            self = self.column(c);
        }
        self
    }

    /// Selects an expression.
    #[must_use]
    pub fn expr(mut self, expr: Expr) -> Self {
        self.items.push(SelectItem::Expr(expr));
        self
    }

    /// Selects an aliased expression.
    #[must_use]
    pub fn expr_as(self, expr: Expr, alias: impl IntoIden) -> Self {
        self.expr(expr.alias(alias))
    }

    /// Whether any item has been selected; without one the statement renders
    /// `SELECT *`.
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Removes all selected items.
    #[must_use]
    pub fn clear_items(mut self) -> Self {
        self.items.clear();
        self
    }

    /// Adds a `FROM table`.
    #[must_use]
    pub fn from(mut self, table: impl Into<TableRef>) -> Self {
        self.from.push(table.into());
        self
    }

    /// Sets `FROM (subquery) AS alias`.
    #[must_use]
    pub fn from_subquery(mut self, subquery: Select, alias: impl IntoIden) -> Self {
        self.from_subquery = Some((Box::new(subquery), alias.into_iden()));
        self
    }

    /// Adds a `JOIN table ON cond` of the given kind.
    #[must_use]
    pub fn join(
        mut self,
        kind: JoinType,
        table: impl Into<TableRef>,
        on: impl IntoCondition,
    ) -> Self {
        self.joins.push(Join {
            kind,
            table: table.into(),
            on: on.into_condition().into_expr(),
        });
        self
    }

    /// Adds an `INNER JOIN`.
    #[must_use]
    pub fn inner_join(self, table: impl Into<TableRef>, on: impl IntoCondition) -> Self {
        self.join(JoinType::Inner, table, on)
    }

    /// Adds a `LEFT JOIN`.
    #[must_use]
    pub fn left_join(self, table: impl Into<TableRef>, on: impl IntoCondition) -> Self {
        self.join(JoinType::Left, table, on)
    }

    /// Adds a condition to `WHERE` with `AND`.
    #[must_use]
    pub fn and_where(mut self, cond: impl IntoCondition) -> Self {
        self.r#where = std::mem::take(&mut self.r#where).add(cond);
        self
    }

    /// Replaces the whole `WHERE` condition.
    #[must_use]
    pub fn cond_where(mut self, cond: impl IntoCondition) -> Self {
        self.r#where = cond.into_condition();
        self
    }

    /// Adds a `GROUP BY` expression.
    #[must_use]
    pub fn group_by(mut self, expr: Expr) -> Self {
        self.group_by.push(expr);
        self
    }

    /// Adds a condition to `HAVING` with `AND`.
    #[must_use]
    pub fn and_having(mut self, cond: impl IntoCondition) -> Self {
        self.having = std::mem::take(&mut self.having).add(cond);
        self
    }

    /// Adds an `ORDER BY column`.
    #[must_use]
    pub fn order_by(mut self, column: impl Into<ColumnRef>, order: Order) -> Self {
        self.order_by.push((Expr::Column(column.into()), order));
        self
    }

    /// Adds an `ORDER BY expr`.
    #[must_use]
    pub fn order_by_expr(mut self, expr: Expr, order: Order) -> Self {
        self.order_by.push((expr, order));
        self
    }

    /// Removes the ordering.
    #[must_use]
    pub fn clear_order_by(mut self) -> Self {
        self.order_by.clear();
        self
    }

    /// Sets `LIMIT`.
    #[must_use]
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Removes `LIMIT`.
    #[must_use]
    pub fn reset_limit(mut self) -> Self {
        self.limit = None;
        self
    }

    /// Sets `OFFSET`.
    #[must_use]
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Removes `OFFSET`.
    #[must_use]
    pub fn reset_offset(mut self) -> Self {
        self.offset = None;
        self
    }

    /// The `FROM` tables.
    pub fn from_tables(&self) -> &[TableRef] {
        &self.from
    }

    /// The join clauses, in order.
    pub fn joins(&self) -> &[Join] {
        &self.joins
    }
}

/// An `ON CONFLICT` clause for inserts.
#[derive(Clone, Debug, PartialEq)]
pub struct OnConflict {
    /// The conflict target columns; empty renders a bare `ON CONFLICT`.
    pub(crate) target: Vec<Ident>,
    /// What to do on conflict.
    pub(crate) action: ConflictAction,
}

/// The action part of an `ON CONFLICT` clause.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ConflictAction {
    /// `DO NOTHING`.
    Nothing,
    /// `DO UPDATE SET column = expr, ...`.
    Update(Vec<(Ident, Expr)>),
}

impl OnConflict {
    /// Builds `ON CONFLICT(columns) DO NOTHING`.
    pub fn do_nothing<C: IntoIden>(columns: impl IntoIterator<Item = C>) -> Self {
        Self {
            target: columns.into_iter().map(IntoIden::into_iden).collect(),
            action: ConflictAction::Nothing,
        }
    }

    /// Builds `ON CONFLICT(columns) DO UPDATE SET c = excluded.c, ...` for
    /// every column in `update`.
    pub fn update_columns<C: IntoIden, U: IntoIden>(
        columns: impl IntoIterator<Item = C>,
        update: impl IntoIterator<Item = U>,
    ) -> Self {
        let sets = update
            .into_iter()
            .map(|c| {
                let c = c.into_iden();
                let excluded =
                    Expr::Column(ColumnRef::TableColumn("excluded".into_iden(), c.clone()));
                (c, excluded)
            })
            .collect();
        Self {
            target: columns.into_iter().map(IntoIden::into_iden).collect(),
            action: ConflictAction::Update(sets),
        }
    }

    /// Builds `ON CONFLICT(columns) DO UPDATE SET` with explicit expressions.
    pub fn update_exprs<C: IntoIden, U: IntoIden>(
        columns: impl IntoIterator<Item = C>,
        sets: impl IntoIterator<Item = (U, Expr)>,
    ) -> Self {
        Self {
            target: columns.into_iter().map(IntoIden::into_iden).collect(),
            action: ConflictAction::Update(
                sets.into_iter().map(|(c, e)| (c.into_iden(), e)).collect(),
            ),
        }
    }
}

/// An `INSERT` statement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Insert {
    /// The target table.
    pub(crate) table: Option<TableRef>,
    /// The column list.
    pub(crate) columns: Vec<Ident>,
    /// The `VALUES` rows, one expression per column.
    pub(crate) rows: Vec<Vec<Expr>>,
    /// Whether to render `DEFAULT VALUES` instead of rows.
    pub(crate) default_values: bool,
    /// The `ON CONFLICT` clause.
    pub(crate) on_conflict: Option<OnConflict>,
    /// The `RETURNING` clause.
    pub(crate) returning: Returning,
    /// A `SELECT` source rendered instead of `VALUES` when set.
    pub(crate) select: Option<Box<Select>>,
}

impl Insert {
    /// Sets `INSERT INTO table`.
    #[must_use]
    pub fn into_table(mut self, table: impl Into<TableRef>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Sets the column list.
    #[must_use]
    pub fn columns<C: IntoIden>(mut self, columns: impl IntoIterator<Item = C>) -> Self {
        self.columns = columns.into_iter().map(IntoIden::into_iden).collect();
        self
    }

    /// Adds a row of values, one per column.
    #[must_use]
    pub fn values<V: Into<Expr>>(mut self, row: impl IntoIterator<Item = V>) -> Self {
        self.rows.push(row.into_iter().map(Into::into).collect());
        self
    }

    /// Sets `INSERT INTO table DEFAULT VALUES`.
    #[must_use]
    pub fn default_values(mut self) -> Self {
        self.default_values = true;
        self
    }

    /// Sets `INSERT INTO table (...) SELECT ...`.
    #[must_use]
    pub fn select_from(mut self, select: Select) -> Self {
        self.select = Some(Box::new(select));
        self
    }

    /// Sets the `ON CONFLICT` clause.
    #[must_use]
    pub fn on_conflict(mut self, on_conflict: OnConflict) -> Self {
        self.on_conflict = Some(on_conflict);
        self
    }

    /// Sets the `RETURNING` clause.
    #[must_use]
    pub fn returning(mut self, returning: Returning) -> Self {
        self.returning = returning;
        self
    }

    /// Sets `RETURNING *`.
    #[must_use]
    pub fn returning_all(self) -> Self {
        self.returning(Returning::All)
    }

    /// The number of rows added so far.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}

/// An `UPDATE` statement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Update {
    /// The target table.
    pub(crate) table: Option<TableRef>,
    /// The `SET` assignments, in order.
    pub(crate) sets: Vec<(Ident, Expr)>,
    /// The `WHERE` condition.
    pub(crate) r#where: Option<Condition>,
    /// The `RETURNING` clause.
    pub(crate) returning: Returning,
    /// The `LIMIT`, written inline since it is not a user value.
    pub(crate) limit: Option<u64>,
}

impl Update {
    /// Sets `UPDATE table`.
    #[must_use]
    pub fn table(mut self, table: impl Into<TableRef>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Adds `SET column = value`.
    #[must_use]
    pub fn value(mut self, column: impl IntoIden, value: impl Into<Expr>) -> Self {
        self.sets.push((column.into_iden(), value.into()));
        self
    }

    /// Adds several `SET` assignments.
    #[must_use]
    pub fn values<C: IntoIden, V: Into<Expr>>(
        mut self,
        sets: impl IntoIterator<Item = (C, V)>,
    ) -> Self {
        for (c, v) in sets {
            self = self.value(c, v);
        }
        self
    }

    /// Adds a condition to `WHERE` with `AND`.
    #[must_use]
    pub fn and_where(mut self, cond: impl IntoCondition) -> Self {
        let current = self.r#where.take().unwrap_or_else(Condition::all);
        self.r#where = Some(current.add(cond));
        self
    }

    /// Sets the `RETURNING` clause.
    #[must_use]
    pub fn returning(mut self, returning: Returning) -> Self {
        self.returning = returning;
        self
    }

    /// Sets `RETURNING *`.
    #[must_use]
    pub fn returning_all(self) -> Self {
        self.returning(Returning::All)
    }

    /// Whether any `SET` assignment was added.
    pub fn has_sets(&self) -> bool {
        !self.sets.is_empty()
    }
}

/// A `DELETE` statement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Delete {
    /// The target table.
    pub(crate) table: Option<TableRef>,
    /// The `WHERE` condition.
    pub(crate) r#where: Option<Condition>,
    /// The `RETURNING` clause.
    pub(crate) returning: Returning,
}

impl Delete {
    /// Sets `DELETE FROM table`.
    #[must_use]
    pub fn from_table(mut self, table: impl Into<TableRef>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Adds a condition to `WHERE` with `AND`.
    #[must_use]
    pub fn and_where(mut self, cond: impl IntoCondition) -> Self {
        let current = self.r#where.take().unwrap_or_else(Condition::all);
        self.r#where = Some(current.add(cond));
        self
    }

    /// Sets the `RETURNING` clause.
    #[must_use]
    pub fn returning(mut self, returning: Returning) -> Self {
        self.returning = returning;
        self
    }
}

impl From<Value> for SelectItem {
    fn from(v: Value) -> Self {
        SelectItem::Expr(Expr::Value(v))
    }
}
