//! The `DELETE` builders, modeled by [`DeleteOne`] and [`DeleteMany`].
//!
//! [`DeleteOne`] is a thin layer over [`DeleteMany`]: it only contributes the
//! primary-key condition read from an active model, so that both paths
//! render through the same statement and report the same
//! [`DeleteResult`]. Both offer `exec_with_returning`, which uses
//! `DELETE ... RETURNING *` to hand back the rows as they were just before
//! removal.

use std::marker::PhantomData;

use turso_orm_driver::ConnectionTrait;
use turso_sql::{Build, IntoCondition, Returning, Statement, Value};

use crate::entity::{ActiveModelTrait, EntityTrait, FromQueryResult};
use crate::query::select::pk_condition;
use crate::{DbErr, Result};

/// The outcome of a delete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeleteResult {
    /// The number of rows deleted.
    pub rows_affected: u64,
}

/// A `DELETE` of one active model, matched by primary key.
#[derive(Clone, Debug)]
pub struct DeleteOne<A: ActiveModelTrait> {
    /// The active model whose primary key selects the row.
    model: A,
}

impl<A: ActiveModelTrait> DeleteOne<A> {
    /// Wraps the active model to delete.
    pub(crate) fn new(model: A) -> Self {
        Self { model }
    }

    /// Executes the delete.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::PrimaryKeyNotSet`] when a key attribute of the model
    /// is `NotSet`; [`DbErr::Driver`] when the statement fails.
    pub async fn exec<C: ConnectionTrait>(self, db: &C) -> Result<DeleteResult> {
        let pk = self
            .model
            .get_primary_key_value()
            .ok_or(DbErr::PrimaryKeyNotSet)?;
        DeleteMany::<A::Entity>::new()
            .filter_by_pk(pk)
            .exec(db)
            .await
    }

    /// Executes the delete with `RETURNING *` and returns the removed row.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::PrimaryKeyNotSet`] when a key attribute of the model
    /// is `NotSet`; [`DbErr::RecordNotFound`] when no row matched;
    /// [`DbErr::Driver`] when the statement fails or the returned row cannot
    /// be decoded.
    pub async fn exec_with_returning<C: ConnectionTrait>(
        self,
        db: &C,
    ) -> Result<<A::Entity as EntityTrait>::Model> {
        let pk = self
            .model
            .get_primary_key_value()
            .ok_or(DbErr::PrimaryKeyNotSet)?;
        DeleteMany::<A::Entity>::new()
            .filter_by_pk(pk)
            .exec_with_returning(db)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| DbErr::RecordNotFound("delete".into()))
    }
}

/// A `DELETE` over any number of rows of an entity.
#[derive(Clone, Debug)]
pub struct DeleteMany<E: EntityTrait> {
    /// The statement being assembled.
    query: turso_sql::Delete,
    /// Ties the builder to its entity without storing it.
    _e: PhantomData<E>,
}

impl<E: EntityTrait> DeleteMany<E> {
    /// Starts `DELETE FROM table` with no condition.
    pub(crate) fn new() -> Self {
        Self {
            query: turso_sql::Query::delete().from_table(E::TABLE_NAME),
            _e: PhantomData,
        }
    }

    /// Restricts the delete to the row whose primary key equals `values`.
    pub(crate) fn filter_by_pk(mut self, values: Vec<Value>) -> Self {
        self.query = self.query.and_where(pk_condition::<E>(values));
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

    /// Executes the delete.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails.
    pub async fn exec<C: ConnectionTrait>(self, db: &C) -> Result<DeleteResult> {
        let result = db.execute(self.build()).await?;
        Ok(DeleteResult {
            rows_affected: result.rows_affected,
        })
    }

    /// Executes the delete with `RETURNING *` and decodes the removed rows.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails or a returned row
    /// cannot be decoded.
    pub async fn exec_with_returning<C: ConnectionTrait>(
        mut self,
        db: &C,
    ) -> Result<Vec<E::Model>> {
        self.query = self.query.returning(Returning::All);
        db.query_all(self.build())
            .await?
            .iter()
            .map(|r| E::Model::from_query_result(r, ""))
            .collect()
    }
}
