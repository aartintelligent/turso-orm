//! The `INSERT` builders, modeled by [`Insert`] and [`InsertMany`].
//!
//! Only `Set` attributes are sent, so that columns left `NotSet` take their
//! database default; an active model with nothing set renders
//! `DEFAULT VALUES`. [`Insert`] offers two execution paths: `exec` returns
//! just the primary key, reconstructed from `last_insert_rowid()` for
//! integer keys, while `exec_with_returning` uses `RETURNING *` and decodes
//! the full stored row, which is the only way to observe defaults and
//! non-integer keys.

use std::marker::PhantomData;

use turso_orm_driver::ConnectionTrait;
use turso_sql::{Build, Expr, OnConflict, Returning, Statement};

use crate::entity::{
    ActiveModelTrait, ColumnTrait, EntityTrait, FromQueryResult, IdenStatic, Iterable,
    PrimaryKeyTrait,
};
use crate::types::TryFromU64;
use crate::{DbErr, Result};

/// The outcome of an insert executed without `RETURNING`.
#[derive(Clone, Debug)]
pub struct InsertResult<E: EntityTrait> {
    /// The primary key of the inserted row.
    pub last_insert_id: <E::PrimaryKey as PrimaryKeyTrait>::ValueType,
}

/// An `INSERT` of one active model.
#[derive(Clone, Debug)]
pub struct Insert<A: ActiveModelTrait> {
    /// The active model whose `Set` attributes are inserted.
    model: A,
    /// The optional `ON CONFLICT` clause.
    on_conflict: Option<OnConflict>,
}

impl<A: ActiveModelTrait> Insert<A> {
    /// Wraps the active model to insert.
    pub(crate) fn one(model: A) -> Self {
        Self {
            model,
            on_conflict: None,
        }
    }

    /// Sets the `ON CONFLICT` clause.
    #[must_use]
    pub fn on_conflict(mut self, on_conflict: OnConflict) -> Self {
        self.on_conflict = Some(on_conflict);
        self
    }

    /// Builds the statement, with or without `RETURNING *`.
    fn statement(&self, returning: bool) -> turso_sql::Insert {
        let mut columns = Vec::new();
        let mut values = Vec::new();
        for c in <<A::Entity as EntityTrait>::Column as Iterable>::iter() {
            if let crate::entity::ActiveValue::Set(v) = self.model.get(c) {
                columns.push(c.as_str());
                values.push(Expr::val(v));
            }
        }
        let mut insert =
            turso_sql::Query::insert().into_table(<A::Entity as EntityTrait>::TABLE_NAME);
        // An empty column list is not valid SQL; `DEFAULT VALUES` is how a
        // row made only of defaults is inserted.
        if columns.is_empty() {
            insert = insert.default_values();
        } else {
            insert = insert.columns(columns).values(values);
        }
        if let Some(oc) = &self.on_conflict {
            insert = insert.on_conflict(oc.clone());
        }
        if returning {
            insert = insert.returning(Returning::All);
        }
        insert
    }

    /// Renders the statement without `RETURNING`.
    pub fn build(&self) -> Statement {
        self.statement(false).to_statement()
    }

    /// Executes the insert and returns the primary key.
    ///
    /// A key the caller supplied is echoed back; a generated one is read
    /// from `last_insert_rowid()`. Non-integer keys cannot be reconstructed
    /// this way, so use [`exec_with_returning`](Self::exec_with_returning)
    /// for them.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::RecordNotInserted`] when the statement inserted
    /// nothing, for example under `ON CONFLICT DO NOTHING`; [`DbErr::Type`]
    /// when the key is not an integer or does not fit the key type;
    /// [`DbErr::Driver`] when the statement fails.
    pub async fn exec<C: ConnectionTrait>(self, db: &C) -> Result<InsertResult<A::Entity>> {
        let result = db.execute(self.build()).await?;
        if result.rows_affected == 0 {
            return Err(DbErr::RecordNotInserted);
        }
        let last_insert_id = if let Some(values) = self.model.get_primary_key_value()
            && !<A::Entity as EntityTrait>::PrimaryKey::auto_increment()
        {
            // The caller supplied the key, so decode it back from the bound
            // values rather than trusting the row id.
            decode_pk::<A::Entity>(&values)?
        } else {
            let id = u64::try_from(result.last_insert_id).unwrap_or_default();
            <<A::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType::try_from_u64(
                id,
            )?
        };
        Ok(InsertResult { last_insert_id })
    }

    /// Executes the insert with `RETURNING *` and returns the stored model.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::RecordNotInserted`] when the statement inserted
    /// nothing; [`DbErr::Driver`] when the statement fails or the returned
    /// row cannot be decoded.
    pub async fn exec_with_returning<C: ConnectionTrait>(
        self,
        db: &C,
    ) -> Result<<A::Entity as EntityTrait>::Model> {
        let row = db
            .query_one(self.statement(true).to_statement())
            .await?
            .ok_or(DbErr::RecordNotInserted)?;
        <A::Entity as EntityTrait>::Model::from_query_result(&row, "")
    }
}

/// Rebuilds a caller-supplied primary key from its bound values.
///
/// Only a single integer key can be rebuilt through `TryFromU64`; other
/// shapes cannot be reconstructed generically, so callers that need them
/// go through `exec_with_returning`.
///
/// # Errors
///
/// Returns [`DbErr::Type`] when the key is composite, not an integer,
/// negative, or does not fit the key type.
fn decode_pk<E: EntityTrait>(
    values: &[turso_sql::Value],
) -> Result<<E::PrimaryKey as PrimaryKeyTrait>::ValueType> {
    match values.first() {
        Some(turso_sql::Value::Integer(n)) if values.len() == 1 => {
            let id = u64::try_from(*n).map_err(|_| DbErr::Type("negative primary key".into()))?;
            <E::PrimaryKey as PrimaryKeyTrait>::ValueType::try_from_u64(id)
        }
        _ => Err(DbErr::Type(
            "non-integer primary keys cannot be returned by exec(); use exec_with_returning()"
                .into(),
        )),
    }
}

/// An `INSERT` of several active models in one statement.
#[derive(Clone, Debug)]
pub struct InsertMany<A: ActiveModelTrait> {
    /// The active models to insert, in order.
    models: Vec<A>,
    /// The optional `ON CONFLICT` clause.
    on_conflict: Option<OnConflict>,
    /// Ties the builder to its active model type.
    _a: PhantomData<A>,
}

impl<A: ActiveModelTrait> InsertMany<A> {
    /// Collects the active models to insert.
    pub(crate) fn many(models: impl IntoIterator<Item = A>) -> Self {
        Self {
            models: models.into_iter().collect(),
            on_conflict: None,
            _a: PhantomData,
        }
    }

    /// Sets the `ON CONFLICT` clause.
    #[must_use]
    pub fn on_conflict(mut self, on_conflict: OnConflict) -> Self {
        self.on_conflict = Some(on_conflict);
        self
    }

    /// Whether there is nothing to insert.
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    /// Builds the statement, or `None` when there are no models.
    fn statement(&self, returning: bool) -> Option<turso_sql::Insert> {
        if self.models.is_empty() {
            return None;
        }
        // A multi-row insert needs one column list, so it is the union of
        // the `Set` columns across models. SQLite cannot ask for the default
        // of one cell, so a model missing a column contributes the default
        // the entity declares for it, or `NULL` when it declares none.
        let columns: Vec<<A::Entity as EntityTrait>::Column> =
            <<A::Entity as EntityTrait>::Column as Iterable>::iter()
                .filter(|c| self.models.iter().any(|m| m.get(*c).is_set()))
                .collect();
        let mut insert = turso_sql::Query::insert()
            .into_table(<A::Entity as EntityTrait>::TABLE_NAME)
            .columns(columns.iter().map(IdenStatic::as_str));
        for m in &self.models {
            let row: Vec<Expr> = columns
                .iter()
                .map(|c| match m.get(*c).into_value() {
                    Some(v) => Expr::val(v),
                    None => c
                        .def()
                        .default
                        .unwrap_or_else(|| Expr::val(turso_sql::Value::Null)),
                })
                .collect();
            insert = insert.values(row);
        }
        if let Some(oc) = &self.on_conflict {
            insert = insert.on_conflict(oc.clone());
        }
        if returning {
            insert = insert.returning(Returning::All);
        }
        Some(insert)
    }

    /// Executes the insert and returns the number of rows inserted.
    ///
    /// With no models, returns zero without touching the database.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails.
    pub async fn exec<C: ConnectionTrait>(self, db: &C) -> Result<u64> {
        match self.statement(false) {
            None => Ok(0),
            Some(stmt) => Ok(db.execute(stmt.to_statement()).await?.rows_affected),
        }
    }

    /// Executes the insert with `RETURNING *` and decodes the stored rows.
    ///
    /// With no models, returns an empty list without touching the database.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when the statement fails or a returned row
    /// cannot be decoded.
    pub async fn exec_with_returning<C: ConnectionTrait>(
        self,
        db: &C,
    ) -> Result<Vec<<A::Entity as EntityTrait>::Model>> {
        match self.statement(true) {
            None => Ok(Vec::new()),
            Some(stmt) => db
                .query_all(stmt.to_statement())
                .await?
                .iter()
                .map(|r| <A::Entity as EntityTrait>::Model::from_query_result(r, ""))
                .collect(),
        }
    }
}
