//! The `SELECT` builders, modeled by [`Select`], [`Selector`], [`SelectTwo`] and [`SelectTwoMany`].
//!
//! [`Select`] is the entity-typed entry point: it knows the `Column` enum of
//! its entity, so filters, ordering and projections are checked at compile
//! time, and it decodes into the entity's model. [`Selector`] is the untyped
//! layer underneath it that decodes any `turso_sql::Select` into a
//! [`FromQueryResult`] type; `into_model` moves from the first to the
//! second. [`Paginator`] and [`RawSelector`] are conveniences over the same
//! execution path.
//!
//! Every column reference is rendered qualified as `table.column`, so that a
//! joined table with a column of the same name never makes the statement
//! ambiguous. A table joined a second time — a self-referencing relation, or
//! a chain that comes back to the base table — is aliased `table_n`, and the
//! join condition is rendered against that alias; [`Select::join_as`] lets a
//! caller pick the alias so that filters can name it.
//!
//! [`SelectTwo`], produced by `find_also_related`, aliases the two column
//! lists as `A_<col>` and `B_<col>`; [`FromQueryResult::from_query_result`]
//! takes that prefix so that both models decode from a single row, and
//! `from_query_result_optional` turns an all-`NULL` `B_` side into `None`.
//! [`SelectTwoMany`] runs the same statement and folds the rows into one
//! entry per left-hand model.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::pin::Pin;

use futures_util::{Stream, TryStreamExt};
use turso_orm_driver::{ConnectionTrait, Row, StreamTrait};
use turso_sql::{
    Build, Condition, Expr, Func, IntoCondition, IntoIden, JoinType, Order, Statement, TableRef,
    Value,
};

use crate::entity::relation::column_of;
use crate::entity::{
    EntityTrait, FromQueryResult, IdenStatic, Iterable, Linked, ModelTrait, PartialModelTrait,
    PrimaryKeyToColumn, Related, RelationDef,
};
use crate::{DbErr, Result};

/// A boxed stream of decoded models, as returned by [`Select::stream`] and
/// [`Selector::stream`].
///
/// Boxing makes the stream `Unpin`, so callers can drive it with
/// `TryStreamExt::try_next` in a `while let` loop without pinning it first.
pub type ModelStream<'a, M> = Pin<Box<dyn Stream<Item = Result<M>> + Send + 'a>>;

/// A `SELECT` on one entity, decoded into its model.
#[derive(Clone, Debug)]
pub struct Select<E: EntityTrait> {
    /// The statement being assembled.
    query: turso_sql::Select,
    /// Ties the builder to its entity without storing it.
    _e: PhantomData<E>,
}

/// Renders a column of `E` as the qualified `table.column` expression.
fn qualified<E: EntityTrait>(column: E::Column) -> Expr {
    Expr::col((E::TABLE_NAME, column.as_str()))
}

/// Builds `target.to_col = <model.from_col>` for every column pair of `rel`,
/// with the target side qualified by `target_ref`.
///
/// `rel.from_tbl` is the model's table. A `from` column the model does not
/// have is compared against `NULL`, which matches nothing, so a relation
/// built by hand with a wrong column yields an empty result rather than a
/// panic.
fn related_condition<S: EntityTrait>(
    rel: &RelationDef,
    model: &S::Model,
    target_ref: &str,
) -> Condition {
    let mut cond = Condition::all();
    for (f, t) in rel.from_col.iter().zip(&rel.to_col) {
        let value = column_of::<S>(f).map_or(Value::Null, |c| model.get(c));
        cond = cond.add(Expr::col((target_ref.to_owned(), *t)).eq(Expr::val(value)));
    }
    cond
}

impl<E: EntityTrait> Select<E> {
    /// Starts `SELECT <every column> FROM table`.
    ///
    /// Columns are listed explicitly rather than as `*` so that joins do not
    /// leak the other table's columns into the decoded row.
    pub(crate) fn new() -> Self {
        let mut query = turso_sql::Select::new().from(E::TABLE_NAME);
        for c in E::Column::iter() {
            query = query.expr(qualified::<E>(c));
        }
        Self {
            query,
            _e: PhantomData,
        }
    }

    /// Restricts the query to the row whose primary key equals `values`.
    pub(crate) fn filter_by_pk(mut self, values: Vec<Value>) -> Self {
        for (pk, value) in E::PrimaryKey::iter().zip(values) {
            let column = pk.into_column();
            self.query = self
                .query
                .and_where(qualified::<E>(column).eq(Expr::val(value)));
        }
        self
    }

    /// Starts a query for the rows of `E` related to `model` of entity `S`.
    ///
    /// A direct relation needs no join: the `WHERE` pins the columns of `E`
    /// that the relation pairs with the model's own. A many-to-many relation
    /// joins the junction table and pins its columns instead.
    pub(crate) fn find_related_to<S>(model: &S::Model) -> Self
    where
        S: EntityTrait + Related<E>,
    {
        let to = <S as Related<E>>::to();
        let mut select = Self::new();
        match <S as Related<E>>::via() {
            Some(via) => {
                let junction = select.join_table(JoinType::Inner, to.from_tbl, |r| {
                    to.join_condition_refs(r, E::TABLE_NAME)
                });
                select.query = select
                    .query
                    .and_where(related_condition::<S>(&via, model, &junction));
            }
            None => {
                select.query =
                    select
                        .query
                        .and_where(related_condition::<S>(&to, model, E::TABLE_NAME));
            }
        }
        select
    }

    /// Starts a query for the rows of `E` reached from `model` through the
    /// chain `link`.
    ///
    /// The hops are joined from the end of the chain back to its start, so
    /// that the base table of the query is `E`; the start entity is joined
    /// last and its primary key pinned in the `WHERE`.
    pub(crate) fn find_linked_to<L>(link: &L, model: &<L::FromEntity as EntityTrait>::Model) -> Self
    where
        L: Linked<ToEntity = E>,
    {
        let mut select = Self::new();
        let mut previous = E::TABLE_NAME.to_owned();
        for hop in link.link().into_iter().rev() {
            previous = select.join_table(JoinType::Inner, hop.from_tbl, |r| {
                hop.join_condition_refs(r, &previous)
            });
        }
        for pk in <L::FromEntity as EntityTrait>::PrimaryKey::iter() {
            let column = pk.into_column();
            select.query = select.query.and_where(
                Expr::col((previous.clone(), column.as_str())).eq(Expr::val(model.get(column))),
            );
        }
        select
    }

    /// Joins `table`, aliasing it `table_n` when it is already part of the
    /// query, and returns the reference the joined side goes by.
    ///
    /// `on` receives that reference and builds the join condition against
    /// it, so a self-join or a chain revisiting a table never renders two
    /// identical qualifiers.
    pub(crate) fn join_table(
        &mut self,
        kind: JoinType,
        table: &'static str,
        on: impl FnOnce(&str) -> Expr,
    ) -> String {
        let occurrences = self
            .query
            .from_tables()
            .iter()
            .chain(self.query.joins().iter().map(|j| &j.table))
            .filter(|t| t.name.name() == table)
            .count();
        let (table_ref, reference) = if occurrences == 0 {
            (TableRef::new(table), table.to_owned())
        } else {
            let alias = format!("{table}_{occurrences}");
            (TableRef::new(table).alias(alias.clone()), alias)
        };
        let cond = on(&reference);
        self.query = std::mem::take(&mut self.query).join(kind, table_ref, cond);
        reference
    }

    /// Joins `R` through the relation `E` declares to it, following the
    /// junction hop first for a many-to-many relation, and returns the
    /// reference of `R`'s table.
    fn join_related<R: EntityTrait>(&mut self, kind: JoinType) -> String
    where
        E: Related<R>,
    {
        let to = <E as Related<R>>::to();
        match <E as Related<R>>::via() {
            Some(via) => {
                let junction = self.join_table(kind, via.to_tbl, |r| {
                    via.join_condition_refs(via.from_tbl, r)
                });
                self.join_table(kind, to.to_tbl, |r| to.join_condition_refs(&junction, r))
            }
            None => self.join_table(kind, to.to_tbl, |r| to.join_condition_refs(to.from_tbl, r)),
        }
    }

    /// Follows every hop of `link` from `E`, joining each table, and returns
    /// the reference of the last one.
    fn join_linked<L: Linked<FromEntity = E>>(&mut self, kind: JoinType, link: &L) -> String {
        let mut previous = E::TABLE_NAME.to_owned();
        for hop in link.link() {
            previous = self.join_table(kind, hop.to_tbl, |r| hop.join_condition_refs(&previous, r));
        }
        previous
    }

    /// Rebuilds the select list as `A_<col>` for `E` and `B_<col>` for `R`
    /// read through `right_ref`, and wraps the query as a [`SelectTwo`].
    fn into_select_two<R: EntityTrait>(self, right_ref: &str) -> SelectTwo<E, R> {
        let mut query = self.query.clear_items();
        for c in E::Column::iter() {
            query = query.expr_as(qualified::<E>(c), format!("A_{}", c.as_str()));
        }
        for c in R::Column::iter() {
            query = query.expr_as(
                Expr::col((right_ref.to_owned(), c.as_str())),
                format!("B_{}", c.as_str()),
            );
        }
        SelectTwo {
            query,
            _e: PhantomData,
        }
    }

    /// Adds a `WHERE` condition, `AND`ed with the previous ones.
    #[must_use]
    pub fn filter(mut self, cond: impl IntoCondition) -> Self {
        self.query = self.query.and_where(cond);
        self
    }

    /// Adds a `WHERE` condition only when `cond` is `Some`.
    #[must_use]
    pub fn filter_option(self, cond: Option<impl IntoCondition>) -> Self {
        match cond {
            Some(c) => self.filter(c),
            None => self,
        }
    }

    /// Restricts the query to the rows of `E` related to `model` through
    /// `rel`, whose `from_tbl` is the model's table and `to_tbl` is `E`'s.
    ///
    /// This is the explicit form of [`ModelTrait::find_related`] for a
    /// relation that is not the one `Related` names: a second relation to
    /// the same entity, or the children side of a self-referencing relation.
    #[must_use]
    pub fn related_to<M: ModelTrait>(mut self, rel: &RelationDef, model: &M) -> Self {
        self.query =
            self.query
                .and_where(related_condition::<M::Entity>(rel, model, E::TABLE_NAME));
        self
    }

    /// Adds `ORDER BY column` in the given direction.
    #[must_use]
    pub fn order_by(mut self, column: E::Column, order: Order) -> Self {
        self.query = self.query.order_by_expr(qualified::<E>(column), order);
        self
    }

    /// Adds `ORDER BY column ASC`.
    #[must_use]
    pub fn order_by_asc(self, column: E::Column) -> Self {
        self.order_by(column, Order::Asc)
    }

    /// Adds `ORDER BY column DESC`.
    #[must_use]
    pub fn order_by_desc(self, column: E::Column) -> Self {
        self.order_by(column, Order::Desc)
    }

    /// Adds `ORDER BY expr` in the given direction.
    #[must_use]
    pub fn order_by_expr(mut self, expr: Expr, order: Order) -> Self {
        self.query = self.query.order_by_expr(expr, order);
        self
    }

    /// Sets `LIMIT`.
    #[must_use]
    pub fn limit(mut self, limit: u64) -> Self {
        self.query = self.query.limit(limit);
        self
    }

    /// Sets `OFFSET`.
    #[must_use]
    pub fn offset(mut self, offset: u64) -> Self {
        self.query = self.query.offset(offset);
        self
    }

    /// Makes the query `SELECT DISTINCT`.
    #[must_use]
    pub fn distinct(mut self) -> Self {
        self.query = self.query.distinct();
        self
    }

    /// Adds `GROUP BY column`.
    #[must_use]
    pub fn group_by(mut self, column: E::Column) -> Self {
        self.query = self.query.group_by(qualified::<E>(column));
        self
    }

    /// Adds `GROUP BY expr`.
    #[must_use]
    pub fn group_by_expr(mut self, expr: Expr) -> Self {
        self.query = self.query.group_by(expr);
        self
    }

    /// Adds a `HAVING` condition, `AND`ed with the previous ones.
    #[must_use]
    pub fn having(mut self, cond: impl IntoCondition) -> Self {
        self.query = self.query.and_having(cond);
        self
    }

    /// Clears the select list, to be rebuilt with [`column`](Self::column),
    /// [`column_as`](Self::column_as) or [`expr_as`](Self::expr_as) and
    /// decoded with [`into_model`](Self::into_model) or
    /// [`into_tuple`](Self::into_tuple).
    #[must_use]
    pub fn select_only(mut self) -> Self {
        self.query = self.query.clear_items();
        self
    }

    /// Adds a column of `E` to the select list.
    #[must_use]
    pub fn column(mut self, column: E::Column) -> Self {
        self.query = self.query.expr(qualified::<E>(column));
        self
    }

    /// Adds a column of `E` to the select list under `alias`.
    #[must_use]
    pub fn column_as(mut self, column: E::Column, alias: impl IntoIden) -> Self {
        self.query = self.query.expr_as(qualified::<E>(column), alias);
        self
    }

    /// Adds an expression to the select list.
    #[must_use]
    pub fn expr(mut self, expr: Expr) -> Self {
        self.query = self.query.expr(expr);
        self
    }

    /// Adds an aliased expression to the select list.
    #[must_use]
    pub fn expr_as(mut self, expr: Expr, alias: impl IntoIden) -> Self {
        self.query = self.query.expr_as(expr, alias);
        self
    }

    /// Joins the target of `rel`, a relation declared on `E`.
    ///
    /// The joined table is aliased `table_n` when it is already part of the
    /// query, as for a self-referencing relation; use
    /// [`join_as`](Self::join_as) to choose the alias.
    #[must_use]
    pub fn join(mut self, kind: JoinType, rel: &RelationDef) -> Self {
        self.join_table(kind, rel.to_tbl, |r| {
            rel.join_condition_refs(rel.from_tbl, r)
        });
        self
    }

    /// Joins the target of `rel` under `alias`, so that filters can qualify
    /// its columns with `Expr::col((alias, "column"))`.
    #[must_use]
    pub fn join_as(mut self, kind: JoinType, rel: &RelationDef, alias: &'static str) -> Self {
        let on = rel.join_condition_refs(rel.from_tbl, alias);
        self.query = self
            .query
            .join(kind, TableRef::new(rel.to_tbl).alias(alias), on);
        self
    }

    /// Joins the entity declaring `rel`, following the relation in reverse.
    #[must_use]
    pub fn join_rev(mut self, kind: JoinType, rel: &RelationDef) -> Self {
        self.join_table(kind, rel.from_tbl, |r| {
            rel.join_condition_refs(r, rel.to_tbl)
        });
        self
    }

    /// Adds `INNER JOIN` to `R` through the relation `E` declares to it,
    /// joining the junction table first for a many-to-many relation.
    #[must_use]
    pub fn inner_join<R: EntityTrait>(mut self, _: R) -> Self
    where
        E: Related<R>,
    {
        self.join_related::<R>(JoinType::Inner);
        self
    }

    /// Adds `LEFT JOIN` to `R` through the relation `E` declares to it,
    /// joining the junction table first for a many-to-many relation.
    #[must_use]
    pub fn left_join<R: EntityTrait>(mut self, _: R) -> Self
    where
        E: Related<R>,
    {
        self.join_related::<R>(JoinType::Left);
        self
    }

    /// Also selects the related `R` row through a `LEFT JOIN`.
    ///
    /// The select list is rebuilt with every column of `E` aliased
    /// `A_<col>` and every column of `R` aliased `B_<col>`, so that both
    /// models can be decoded from one row even when they share column names.
    /// For a many-to-many relation a row is produced per junction entry.
    #[must_use]
    pub fn find_also_related<R: EntityTrait>(mut self, _: R) -> SelectTwo<E, R>
    where
        E: Related<R>,
    {
        let right = self.join_related::<R>(JoinType::Left);
        self.into_select_two::<R>(&right)
    }

    /// Selects every `E` row together with all of its related `R` rows.
    ///
    /// The statement is the one of [`find_also_related`](Self::find_also_related);
    /// the rows are folded so that each `E` model appears once, followed by
    /// the related models in row order.
    #[must_use]
    pub fn find_with_related<R: EntityTrait>(self, r: R) -> SelectTwoMany<E, R>
    where
        E: Related<R>,
    {
        SelectTwoMany {
            inner: self.find_also_related(r),
        }
    }

    /// Also selects the row at the end of the chain `link`, through `LEFT JOIN`s.
    ///
    /// Every table on the path is joined; only the first and the last are
    /// selected, as `A_<col>` and `B_<col>`.
    #[must_use]
    pub fn find_also_linked<L>(mut self, link: &L) -> SelectTwo<E, L::ToEntity>
    where
        L: Linked<FromEntity = E>,
    {
        let right = self.join_linked(JoinType::Left, link);
        self.into_select_two::<L::ToEntity>(&right)
    }

    /// Selects every `E` row together with all the rows at the end of `link`.
    #[must_use]
    pub fn find_with_linked<L>(self, link: &L) -> SelectTwoMany<E, L::ToEntity>
    where
        L: Linked<FromEntity = E>,
    {
        SelectTwoMany {
            inner: self.find_also_linked(link),
        }
    }

    /// Decodes rows into `M` instead of the entity model.
    pub fn into_model<M: FromQueryResult>(self) -> Selector<M> {
        Selector {
            query: self.query,
            _m: PhantomData,
        }
    }

    /// Replaces the select list with the columns `P` reads and decodes rows
    /// into it.
    pub fn into_partial_model<P: PartialModelTrait>(self) -> Selector<P> {
        Selector {
            query: P::select_cols(self.query.clear_items()),
            _m: PhantomData,
        }
    }

    /// Decodes rows into a tuple, by position in the select list.
    ///
    /// Meant to follow [`select_only`](Self::select_only) with as many
    /// items as the tuple has fields.
    pub fn into_tuple<T: FromQueryResult>(self) -> Selector<T> {
        self.into_model::<T>()
    }

    /// Decodes rows into JSON objects keyed by column name.
    #[cfg(feature = "with-json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "with-json")))]
    pub fn into_json(self) -> Selector<serde_json::Value> {
        self.into_model::<serde_json::Value>()
    }

    /// Runs `statement` as given and decodes its rows into the entity model.
    ///
    /// The builder's own clauses are discarded; this is the escape hatch for
    /// a `SELECT` the API cannot express.
    pub fn from_raw_sql(self, statement: Statement) -> RawSelector<E::Model> {
        Selector::<E::Model>::from_statement(statement)
    }

    /// The underlying SQL builder.
    pub fn as_query(&self) -> &turso_sql::Select {
        &self.query
    }

    /// Mutable access to the underlying SQL builder, for clauses this API does not expose.
    pub fn query_mut(&mut self) -> &mut turso_sql::Select {
        &mut self.query
    }

    /// Takes the underlying SQL builder out of the select.
    pub fn into_query(self) -> turso_sql::Select {
        self.query
    }

    /// Renders the statement.
    pub fn build(&self) -> Statement {
        self.query.to_statement()
    }

    /// Fetches the first row, adding `LIMIT 1`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query fails or the row cannot be decoded.
    pub async fn one<C: ConnectionTrait>(self, db: &C) -> Result<Option<E::Model>> {
        self.into_model::<E::Model>().one(db).await
    }

    /// Fetches every row.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query fails or a row cannot be decoded.
    pub async fn all<C: ConnectionTrait>(self, db: &C) -> Result<Vec<E::Model>> {
        self.into_model::<E::Model>().all(db).await
    }

    /// Streams rows as they are read.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query cannot be started; decoding
    /// failures surface as items of the stream.
    pub async fn stream<C: StreamTrait>(self, db: &C) -> Result<ModelStream<'_, E::Model>> {
        self.into_model::<E::Model>().stream(db).await
    }

    /// Counts the rows this query would return.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::RecordNotFound`] when the count query returns no
    /// row; [`DbErr::Driver`] when the query fails.
    pub async fn count<C: ConnectionTrait>(self, db: &C) -> Result<u64> {
        self.into_model::<E::Model>().count(db).await
    }

    /// Whether this query would return at least one row, through
    /// `SELECT EXISTS (query)`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::RecordNotFound`] when the probe returns no row;
    /// [`DbErr::Driver`] when the query fails.
    pub async fn exists<C: ConnectionTrait>(self, db: &C) -> Result<bool> {
        self.into_model::<E::Model>().exists(db).await
    }

    /// Paginates with `page_size` rows per page.
    pub fn paginate<C: ConnectionTrait>(
        self,
        db: &C,
        page_size: u64,
    ) -> Paginator<'_, C, E::Model> {
        self.into_model::<E::Model>().paginate(db, page_size)
    }

    /// Paginates by key over `columns`, most significant first; see
    /// [`Cursor`](crate::query::Cursor).
    ///
    /// Any ordering already on the query is replaced by the key order.
    pub fn cursor_by<const N: usize>(
        self,
        columns: [E::Column; N],
    ) -> crate::query::Cursor<E, E::Model> {
        crate::query::Cursor::new(self.query, columns.to_vec())
    }
}

/// A `SELECT` decoded into an arbitrary [`FromQueryResult`] type.
#[derive(Clone, Debug)]
pub struct Selector<M> {
    /// The statement being assembled.
    query: turso_sql::Select,
    /// Ties the selector to its result type without storing it.
    _m: PhantomData<M>,
}

impl<M: FromQueryResult> Selector<M> {
    /// Wraps any SQL select.
    pub fn from_query(query: turso_sql::Select) -> Self {
        Self {
            query,
            _m: PhantomData,
        }
    }

    /// Wraps a raw statement.
    pub fn from_statement(statement: Statement) -> RawSelector<M> {
        RawSelector {
            statement,
            _m: PhantomData,
        }
    }

    /// Renders the statement.
    pub fn build(&self) -> Statement {
        self.query.to_statement()
    }

    /// Fetches the first row, adding `LIMIT 1`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query fails or the row cannot be decoded.
    pub async fn one<C: ConnectionTrait>(mut self, db: &C) -> Result<Option<M>> {
        self.query = self.query.limit(1);
        let row = db.query_one(self.build()).await?;
        row.map(|r| M::from_query_result(&r, "")).transpose()
    }

    /// Fetches every row.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query fails or a row cannot be decoded.
    pub async fn all<C: ConnectionTrait>(self, db: &C) -> Result<Vec<M>> {
        let rows = db.query_all(self.build()).await?;
        rows.iter().map(|r| M::from_query_result(r, "")).collect()
    }

    /// Streams rows as they are read.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query cannot be started; decoding
    /// failures surface as items of the stream.
    #[allow(
        clippy::needless_lifetimes,
        reason = "the stream borrows `db`, not `self`, and the explicit lifetime says so"
    )]
    pub async fn stream<'a, C: StreamTrait>(self, db: &'a C) -> Result<ModelStream<'a, M>>
    where
        M: 'a,
    {
        let stream = db.stream(self.build()).await?;
        Ok(Box::pin(stream.map_err(DbErr::from).and_then(
            |row| async move { M::from_query_result(&row, "") },
        )))
    }

    /// The query without ordering, limit and offset, for probes that only
    /// care about the row set.
    fn unordered(&self) -> turso_sql::Select {
        self.query
            .clone()
            .clear_order_by()
            .reset_limit()
            .reset_offset()
    }

    /// Counts the rows with `SELECT COUNT(*) FROM (query)`.
    ///
    /// Ordering, limit and offset are stripped from the inner query because
    /// they do not affect the total and would only slow the count down.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::RecordNotFound`] when the count query returns no
    /// row; [`DbErr::Driver`] when the query fails.
    pub async fn count<C: ConnectionTrait>(self, db: &C) -> Result<u64> {
        let outer = turso_sql::Select::new()
            .expr_as(Func::count_star(), "num_items")
            .from_subquery(self.unordered(), "sub");
        let row = db
            .query_one(outer.to_statement())
            .await?
            .ok_or(DbErr::RecordNotFound("count".into()))?;
        Ok(row.get::<u64>("num_items")?)
    }

    /// Whether the query returns at least one row, through `SELECT EXISTS (query)`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::RecordNotFound`] when the probe returns no row;
    /// [`DbErr::Driver`] when the query fails.
    pub async fn exists<C: ConnectionTrait>(self, db: &C) -> Result<bool> {
        let probe = turso_sql::Select::new().expr_as(Expr::exists(self.unordered()), "found");
        let row = db
            .query_one(probe.to_statement())
            .await?
            .ok_or(DbErr::RecordNotFound("exists".into()))?;
        Ok(row.get::<bool>("found")?)
    }

    /// Paginates with `page_size` rows per page; a page size of zero is treated as one.
    pub fn paginate<C: ConnectionTrait>(self, db: &C, page_size: u64) -> Paginator<'_, C, M> {
        Paginator {
            query: self.query,
            page_size: page_size.max(1),
            db,
            _m: PhantomData,
        }
    }
}

/// A raw statement decoded into `M`.
#[derive(Clone, Debug)]
pub struct RawSelector<M> {
    /// The statement to run as given.
    statement: Statement,
    /// Ties the selector to its result type without storing it.
    _m: PhantomData<M>,
}

impl<M: FromQueryResult> RawSelector<M> {
    /// Fetches the first row the statement returns.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query fails or the row cannot be decoded.
    pub async fn one<C: ConnectionTrait>(self, db: &C) -> Result<Option<M>> {
        let row = db.query_one(self.statement).await?;
        row.map(|r| M::from_query_result(&r, "")).transpose()
    }

    /// Fetches every row the statement returns.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query fails or a row cannot be decoded.
    pub async fn all<C: ConnectionTrait>(self, db: &C) -> Result<Vec<M>> {
        let rows = db.query_all(self.statement).await?;
        rows.iter().map(|r| M::from_query_result(r, "")).collect()
    }
}

/// Page-by-page access to a query.
#[derive(Debug)]
pub struct Paginator<'db, C, M> {
    /// The query without `LIMIT` and `OFFSET`; each page adds its own.
    query: turso_sql::Select,
    /// The number of rows per page, at least one.
    page_size: u64,
    /// The connection pages are fetched through.
    db: &'db C,
    /// Ties the paginator to its result type without storing it.
    _m: PhantomData<M>,
}

impl<C: ConnectionTrait, M: FromQueryResult> Paginator<'_, C, M> {
    /// Fetches the rows of page `page`, counted from zero.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query fails or a row cannot be decoded.
    pub async fn fetch_page(&self, page: u64) -> Result<Vec<M>> {
        let query = self
            .query
            .clone()
            .limit(self.page_size)
            .offset(page.saturating_mul(self.page_size));
        Selector::<M>::from_query(query).all(self.db).await
    }

    /// The total number of rows.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::RecordNotFound`] when the count query returns no
    /// row; [`DbErr::Driver`] when the query fails.
    pub async fn num_items(&self) -> Result<u64> {
        Selector::<M>::from_query(self.query.clone())
            .count(self.db)
            .await
    }

    /// The total number of pages.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`num_items`](Self::num_items).
    pub async fn num_pages(&self) -> Result<u64> {
        let items = self.num_items().await?;
        Ok(items.div_ceil(self.page_size))
    }

    /// Both totals from a single count query.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`num_items`](Self::num_items).
    pub async fn num_items_and_pages(&self) -> Result<(u64, u64)> {
        let items = self.num_items().await?;
        Ok((items, items.div_ceil(self.page_size)))
    }

    /// Fetches every page in turn, calling `f` with each one, until a page
    /// comes back short or `f` returns `false`.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`fetch_page`](Self::fetch_page).
    pub async fn for_each_page(&self, mut f: impl FnMut(Vec<M>) -> bool) -> Result<()> {
        let mut page = 0;
        loop {
            let rows = self.fetch_page(page).await?;
            let full = u64::try_from(rows.len()).unwrap_or(u64::MAX) == self.page_size;
            if !f(rows) || !full {
                return Ok(());
            }
            page += 1;
        }
    }
}

/// A `SELECT` of an entity together with an optional related row.
#[derive(Clone, Debug)]
pub struct SelectTwo<E: EntityTrait, R: EntityTrait> {
    /// The statement being assembled, with `A_` and `B_` aliased columns.
    query: turso_sql::Select,
    /// Ties the builder to both entities without storing them.
    _e: PhantomData<(E, R)>,
}

impl<E: EntityTrait, R: EntityTrait> SelectTwo<E, R> {
    /// Adds a `WHERE` condition, `AND`ed with the previous ones.
    #[must_use]
    pub fn filter(mut self, cond: impl IntoCondition) -> Self {
        self.query = self.query.and_where(cond);
        self
    }

    /// Adds `ORDER BY` a column of `E` in the given direction.
    #[must_use]
    pub fn order_by(mut self, column: E::Column, order: Order) -> Self {
        self.query = self.query.order_by_expr(qualified::<E>(column), order);
        self
    }

    /// Adds `ORDER BY` a column of `R` in the given direction.
    ///
    /// The column is qualified by `R`'s table name, which is right unless
    /// `R` was joined under an alias; then use
    /// [`order_by_expr`](Self::order_by_expr) with `Expr::col((alias, column))`.
    #[must_use]
    pub fn order_by_related(mut self, column: R::Column, order: Order) -> Self {
        self.query = self.query.order_by_expr(qualified::<R>(column), order);
        self
    }

    /// Adds `ORDER BY expr` in the given direction.
    #[must_use]
    pub fn order_by_expr(mut self, expr: Expr, order: Order) -> Self {
        self.query = self.query.order_by_expr(expr, order);
        self
    }

    /// Sets `LIMIT`.
    #[must_use]
    pub fn limit(mut self, limit: u64) -> Self {
        self.query = self.query.limit(limit);
        self
    }

    /// Sets `OFFSET`.
    #[must_use]
    pub fn offset(mut self, offset: u64) -> Self {
        self.query = self.query.offset(offset);
        self
    }

    /// Mutable access to the underlying SQL builder, for clauses this API does not expose.
    pub fn query_mut(&mut self) -> &mut turso_sql::Select {
        &mut self.query
    }

    /// Renders the statement.
    pub fn build(&self) -> Statement {
        self.query.to_statement()
    }

    /// Decodes one row into the `E` model and the optional `R` model.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when either side cannot be decoded.
    fn decode(row: &Row) -> Result<(E::Model, Option<R::Model>)> {
        let a = E::Model::from_query_result(row, "A_")?;
        let b = R::Model::from_query_result_optional(row, "B_")?;
        Ok((a, b))
    }

    /// Fetches the first pair, adding `LIMIT 1`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query fails or the row cannot be decoded.
    pub async fn one<C: ConnectionTrait>(
        mut self,
        db: &C,
    ) -> Result<Option<(E::Model, Option<R::Model>)>> {
        self.query = self.query.limit(1);
        db.query_one(self.build())
            .await?
            .as_ref()
            .map(Self::decode)
            .transpose()
    }

    /// Fetches every pair.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query fails or a row cannot be decoded.
    pub async fn all<C: ConnectionTrait>(
        self,
        db: &C,
    ) -> Result<Vec<(E::Model, Option<R::Model>)>> {
        db.query_all(self.build())
            .await?
            .iter()
            .map(Self::decode)
            .collect()
    }
}

/// A `SELECT` of an entity together with all of its related rows.
///
/// Produced by [`Select::find_with_related`] and
/// [`Select::find_with_linked`]. The statement is the joined one of
/// [`SelectTwo`]; [`all`](Self::all) folds the result so that each `E`
/// model, identified by its primary key, appears once with the related
/// models collected in row order. `LIMIT` and `OFFSET` are deliberately not
/// offered because they would count joined rows, not `E` models.
#[derive(Clone, Debug)]
pub struct SelectTwoMany<E: EntityTrait, R: EntityTrait> {
    /// The joined statement.
    inner: SelectTwo<E, R>,
}

impl<E: EntityTrait, R: EntityTrait> SelectTwoMany<E, R> {
    /// Adds a `WHERE` condition, `AND`ed with the previous ones.
    #[must_use]
    pub fn filter(mut self, cond: impl IntoCondition) -> Self {
        self.inner = self.inner.filter(cond);
        self
    }

    /// Adds `ORDER BY` a column of `E` in the given direction.
    #[must_use]
    pub fn order_by(mut self, column: E::Column, order: Order) -> Self {
        self.inner = self.inner.order_by(column, order);
        self
    }

    /// Adds `ORDER BY` a column of `R` in the given direction.
    #[must_use]
    pub fn order_by_related(mut self, column: R::Column, order: Order) -> Self {
        self.inner = self.inner.order_by_related(column, order);
        self
    }

    /// Adds `ORDER BY expr` in the given direction.
    #[must_use]
    pub fn order_by_expr(mut self, expr: Expr, order: Order) -> Self {
        self.inner = self.inner.order_by_expr(expr, order);
        self
    }

    /// Renders the statement.
    pub fn build(&self) -> Statement {
        self.inner.build()
    }

    /// Fetches every `E` model with its related `R` models.
    ///
    /// Models are returned in order of first appearance, so an `ORDER BY`
    /// on `E` orders the groups and one on `R` orders within each group.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the query fails or a row cannot be decoded.
    pub async fn all<C: ConnectionTrait>(self, db: &C) -> Result<Vec<(E::Model, Vec<R::Model>)>> {
        let pairs = self.inner.all(db).await?;
        let mut groups: Vec<(E::Model, Vec<R::Model>)> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        for (left, right) in pairs {
            let key = E::PrimaryKey::iter()
                .map(|pk| left.get(pk.into_column()).to_literal())
                .collect::<Vec<_>>()
                .join("\u{1f}");
            let at = if let Some(&at) = index.get(&key) {
                at
            } else {
                groups.push((left, Vec::new()));
                index.insert(key, groups.len() - 1);
                groups.len() - 1
            };
            if let Some(r) = right {
                groups[at].1.push(r);
            }
        }
        Ok(groups)
    }
}

/// Builds the `pk1 = ? AND pk2 = ?` condition matching one primary key.
///
/// Values are paired with the key columns by position, so `values` must be
/// in key-column order.
pub(crate) fn pk_condition<E: EntityTrait>(values: Vec<Value>) -> Condition {
    let mut cond = Condition::all();
    for (pk, value) in E::PrimaryKey::iter().zip(values) {
        cond = cond.add(qualified::<E>(pk.into_column()).eq(Expr::val(value)));
    }
    cond
}
