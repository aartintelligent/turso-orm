//! Keyset pagination, modeled by [`Cursor`].
//!
//! Offset pagination re-reads and discards every row before the page, and
//! a page can shift when rows are inserted ahead of it. A cursor instead
//! remembers the key of the last row seen and asks for the rows after it,
//! so each page costs an index seek and stays stable under concurrent
//! writes. The key is one or more columns of the entity, usually the primary
//! key or a unique ordering column plus the primary key as a tie-breaker.
//!
//! A composite key is compared lexicographically through the expanded form
//! `a > ?1 OR (a = ?1 AND b > ?2)` rather than a row-value comparison, so
//! the statement only uses operators every SQLite build accepts. The key
//! columns are given as an array, `[Column::Name, Column::Id]`, and the
//! boundary values as a matching tuple.

use std::marker::PhantomData;

use turso_orm_driver::ConnectionTrait;
use turso_sql::{Condition, Expr, Order, Statement, Value};

use crate::Result;
use crate::entity::{ColumnTrait, EntityTrait, FromQueryResult, IdenStatic};
use crate::query::select::Selector;
use crate::types::IntoValueTuple;

/// A keyset-paginated `SELECT`.
///
/// Built by [`Select::cursor_by`](crate::query::Select::cursor_by). Set the
/// boundary with [`after`](Self::after) or [`before`](Self::before), the
/// page size and direction with [`first`](Self::first) or
/// [`last`](Self::last), then run [`all`](Self::all).
#[derive(Clone, Debug)]
pub struct Cursor<E: EntityTrait, M> {
    /// The query without boundary, ordering and limit.
    query: turso_sql::Select,
    /// The key columns, most significant first.
    columns: Vec<E::Column>,
    /// The exclusive lower bound, as one value per key column.
    after: Option<Vec<Value>>,
    /// The exclusive upper bound, as one value per key column.
    before: Option<Vec<Value>>,
    /// The page size and whether it is taken from the start (`true`) or the
    /// end (`false`) of the key range.
    page: Option<(u64, bool)>,
    /// Whether the key is ordered descending.
    descending: bool,
    /// Ties the cursor to its entity and result type without storing them.
    _m: PhantomData<(E, M)>,
}

impl<E: EntityTrait, M: FromQueryResult> Cursor<E, M> {
    /// Wraps a query with the key columns it is paginated by.
    pub(crate) fn new(query: turso_sql::Select, columns: Vec<E::Column>) -> Self {
        Self {
            query,
            columns,
            after: None,
            before: None,
            page: None,
            descending: false,
            _m: PhantomData,
        }
    }

    /// Returns rows whose key is strictly greater than `key`.
    #[must_use]
    pub fn after(mut self, key: impl IntoValueTuple) -> Self {
        self.after = Some(key.into_value_tuple());
        self
    }

    /// Returns rows whose key is strictly less than `key`.
    #[must_use]
    pub fn before(mut self, key: impl IntoValueTuple) -> Self {
        self.before = Some(key.into_value_tuple());
        self
    }

    /// Takes the first `n` rows of the key range, in key order.
    #[must_use]
    pub fn first(mut self, n: u64) -> Self {
        self.page = Some((n, true));
        self
    }

    /// Takes the last `n` rows of the key range, still returned in key order.
    ///
    /// The statement reads them in reverse order with `LIMIT n` and the
    /// result is reversed in memory, which is the only way to take the tail
    /// of a range without knowing its size.
    #[must_use]
    pub fn last(mut self, n: u64) -> Self {
        self.page = Some((n, false));
        self
    }

    /// Orders the key ascending, the default.
    #[must_use]
    pub fn asc(mut self) -> Self {
        self.descending = false;
        self
    }

    /// Orders the key descending, so that `after` moves towards smaller keys.
    #[must_use]
    pub fn desc(mut self) -> Self {
        self.descending = true;
        self
    }

    /// Decodes rows into `T` instead of the current result type.
    pub fn into_model<T: FromQueryResult>(self) -> Cursor<E, T> {
        Cursor {
            query: self.query,
            columns: self.columns,
            after: self.after,
            before: self.before,
            page: self.page,
            descending: self.descending,
            _m: PhantomData,
        }
    }

    /// Builds `(a > v1) OR (a = v1 AND b > v2) OR ...`, or the mirror with
    /// `<`, for the key boundary `values`.
    fn boundary(&self, values: &[Value], greater: bool) -> Condition {
        let mut any = Condition::any();
        for (i, column) in self.columns.iter().enumerate() {
            let mut all = Condition::all();
            for (prefix, value) in self.columns.iter().zip(values).take(i) {
                all = all.add(prefix.into_expr().eq(Expr::val(value.clone())));
            }
            let Some(value) = values.get(i) else {
                break;
            };
            let bound = if greater {
                column.into_expr().gt(Expr::val(value.clone()))
            } else {
                column.into_expr().lt(Expr::val(value.clone()))
            };
            any = any.add(all.add(bound));
        }
        any
    }

    /// Assembles the statement; the second value says whether the rows come
    /// back in reverse key order and must be flipped.
    fn assemble(&self) -> (turso_sql::Select, bool) {
        let mut query = self.query.clone().clear_order_by();
        // Under a descending key, "after" means smaller and "before" larger.
        if let Some(after) = &self.after {
            query = query.and_where(self.boundary(after, !self.descending));
        }
        if let Some(before) = &self.before {
            query = query.and_where(self.boundary(before, self.descending));
        }
        let from_start = self.page.is_none_or(|(_, from_start)| from_start);
        let forward = from_start != self.descending;
        let order = if forward { Order::Asc } else { Order::Desc };
        for column in &self.columns {
            query = query.order_by_expr(column.into_expr(), order);
        }
        if let Some((n, _)) = self.page {
            query = query.limit(n);
        }
        (query, !from_start)
    }

    /// Renders the statement.
    pub fn build(&self) -> Statement {
        turso_sql::Build::to_statement(&self.assemble().0)
    }

    /// Fetches the page, in key order.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`](crate::DbErr::Driver) when the query fails
    /// or a row cannot be decoded.
    pub async fn all<C: ConnectionTrait>(&self, db: &C) -> Result<Vec<M>> {
        let (query, reversed) = self.assemble();
        let mut rows = Selector::<M>::from_query(query).all(db).await?;
        if reversed {
            rows.reverse();
        }
        Ok(rows)
    }

    /// The names of the key columns, most significant first.
    pub fn key_columns(&self) -> Vec<&'static str> {
        self.columns.iter().map(IdenStatic::as_str).collect()
    }
}
