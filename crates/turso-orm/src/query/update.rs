//! The `UPDATE` builders, modeled by [`UpdateOne`] and [`UpdateMany`].
//!
//! [`UpdateOne`] writes the `Set` attributes of an active model back to the
//! row its primary key names and returns the stored model through
//! `RETURNING *`. [`UpdateMany`] is the free-form counterpart for bulk
//! updates driven by an arbitrary `WHERE`.
//!
//! Both builders treat "nothing to set" as a no-op rather than rendering an
//! `UPDATE` without a `SET` clause, which SQLite would reject; `UpdateOne`
//! fetches the row instead so that callers still get a model back.

use std::marker::PhantomData;

use turso_orm_driver::ConnectionTrait;
use turso_sql::{Build, Expr, IntoCondition, Returning, Statement, Value};

use crate::entity::{
    ActiveModelTrait, ActiveValue, EntityTrait, FromQueryResult, IdenStatic, Iterable,
};
use crate::query::select::pk_condition;
use crate::{DbErr, Result};

/// The outcome of an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    /// The number of rows changed.
    pub rows_affected: u64,
}

/// An `UPDATE` of one active model, matched by primary key.
#[derive(Clone, Debug)]
pub struct UpdateOne<A: ActiveModelTrait> {
    /// The active model whose `Set` attributes are written.
    model: A,
}

impl<A: ActiveModelTrait> UpdateOne<A> {
    /// Wraps the active model to update.
    pub(crate) fn new(model: A) -> Self {
        Self { model }
    }

    /// Builds the statement, or `None` when no attribute is `Set`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::PrimaryKeyNotSet`] when a key attribute is `NotSet`.
    fn statement(&self) -> Result<Option<turso_sql::Update>> {
        let pk = self
            .model
            .get_primary_key_value()
            .ok_or(DbErr::PrimaryKeyNotSet)?;
        let mut update = turso_sql::Query::update().table(<A::Entity as EntityTrait>::TABLE_NAME);
        let mut any = false;
        for c in <<A::Entity as EntityTrait>::Column as Iterable>::iter() {
            if let ActiveValue::Set(v) = self.model.get(c) {
                update = update.value(c.as_str(), Expr::val(v));
                any = true;
            }
        }
        if !any {
            return Ok(None);
        }
        update = update
            .and_where(pk_condition::<A::Entity>(pk))
            .returning(Returning::All);
        Ok(Some(update))
    }

    /// Executes the update and returns the stored model.
    ///
    /// With no `Set` attribute the row is fetched instead of updated, so the
    /// result is the same either way.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::PrimaryKeyNotSet`] when a key attribute is `NotSet`;
    /// [`DbErr::RecordNotUpdated`] when no row matches the key;
    /// [`DbErr::Driver`] when the statement fails or the returned row cannot
    /// be decoded.
    pub async fn exec<C: ConnectionTrait>(
        self,
        db: &C,
    ) -> Result<<A::Entity as EntityTrait>::Model> {
        if let Some(update) = self.statement()? {
            let row = db
                .query_one(update.to_statement())
                .await?
                .ok_or(DbErr::RecordNotUpdated)?;
            <A::Entity as EntityTrait>::Model::from_query_result(&row, "")
        } else {
            let pk = self
                .model
                .get_primary_key_value()
                .ok_or(DbErr::PrimaryKeyNotSet)?;
            crate::query::Select::<A::Entity>::new()
                .filter_by_pk(pk)
                .one(db)
                .await?
                .ok_or(DbErr::RecordNotUpdated)
        }
    }
}

/// An `UPDATE` over any number of rows of an entity.
#[derive(Clone, Debug)]
pub struct UpdateMany<E: EntityTrait> {
    /// The statement being assembled.
    query: turso_sql::Update,
    /// Ties the builder to its entity without storing it.
    _e: PhantomData<E>,
}

impl<E: EntityTrait> UpdateMany<E> {
    /// Starts `UPDATE table` with no `SET` and no condition.
    pub(crate) fn new() -> Self {
        Self {
            query: turso_sql::Query::update().table(E::TABLE_NAME),
            _e: PhantomData,
        }
    }

    /// Adds `SET column = expr`.
    #[must_use]
    pub fn col_expr(mut self, column: E::Column, value: impl Into<Expr>) -> Self {
        self.query = self.query.value(column.as_str(), value);
        self
    }

    /// Adds `SET column = value` from a plain value.
    #[must_use]
    pub fn col(self, column: E::Column, value: impl Into<Value>) -> Self {
        self.col_expr(column, Expr::val(value))
    }

    /// Adds a `SET` for every `Set` attribute of `model`.
    #[must_use]
    pub fn set<A: ActiveModelTrait<Entity = E>>(mut self, model: &A) -> Self {
        for c in E::Column::iter() {
            if let ActiveValue::Set(v) = model.get(c) {
                self.query = self.query.value(c.as_str(), Expr::val(v));
            }
        }
        self
    }

    /// Adds a `WHERE` condition, `AND`ed with the previous ones.
    #[must_use]
    pub fn filter(mut self, cond: impl IntoCondition) -> Self {
        self.query = self.query.and_where(cond);
        self
    }

    /// Renders the statement.
    pub fn build(&self) -> Statement {
        self.query.to_statement()
    }

    /// Executes the update; with nothing to set, reports zero rows without touching the database.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails.
    pub async fn exec<C: ConnectionTrait>(self, db: &C) -> Result<UpdateResult> {
        if !self.query.has_sets() {
            return Ok(UpdateResult { rows_affected: 0 });
        }
        let result = db.execute(self.build()).await?;
        Ok(UpdateResult {
            rows_affected: result.rows_affected,
        })
    }

    /// Executes the update with `RETURNING *` and decodes the changed rows.
    ///
    /// With nothing to set, returns an empty list without touching the database.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails or a returned row
    /// cannot be decoded.
    pub async fn exec_with_returning<C: ConnectionTrait>(
        mut self,
        db: &C,
    ) -> Result<Vec<E::Model>> {
        if !self.query.has_sets() {
            return Ok(Vec::new());
        }
        self.query = self.query.returning(Returning::All);
        db.query_all(self.build())
            .await?
            .iter()
            .map(|r| E::Model::from_query_result(r, ""))
            .collect()
    }
}
