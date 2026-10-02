//! Models and row decoding, modeled by [`ModelTrait`] and [`FromQueryResult`].
//!
//! A model is the plain, immutable snapshot of one row; the mutable,
//! change-tracking counterpart lives in `active_model`. Decoding is driven
//! by column name rather than position so that a model can be read out of
//! any result set that happens to contain its columns, including the aliased
//! `A_<col>` / `B_<col>` lists that `find_also_related` emits — that is why
//! [`FromQueryResult::from_query_result`] takes a `prefix`. Tuples are the
//! one positional exception: they decode the select list in order, which is
//! what `into_tuple` relies on after `select_only`.
//!
//! [`ModelTrait::set`] returns a `Result` instead of panicking when the
//! value does not decode as the field type, so that generic code can treat a
//! bad value like any other error.

use async_trait::async_trait;
use turso_orm_driver::{ConnectionTrait, FromValue, Row};
use turso_sql::Value;

use super::active_model::{ActiveModelBehavior, ActiveModelTrait, IntoActiveModel};
use super::base_entity::EntityTrait;
use super::relation::{Linked, Related};
use crate::query::{DeleteResult, Select};
use crate::{DbErr, Result};

/// A row of an entity as a plain struct, derived by `DeriveEntityModel`.
#[async_trait]
pub trait ModelTrait: Clone + Send + Sync + std::fmt::Debug {
    /// The entity this model belongs to.
    type Entity: EntityTrait<Model = Self>;

    /// Reads a column as a SQL value.
    fn get(&self, column: <Self::Entity as EntityTrait>::Column) -> Value;

    /// Writes a column from a SQL value.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Type`] when the value cannot be decoded as the field type.
    fn set(&mut self, column: <Self::Entity as EntityTrait>::Column, value: Value) -> Result<()>;

    /// Builds a query for the rows of `R` related to this model, following
    /// the junction table of a many-to-many relation when there is one.
    fn find_related<R>(&self, _: R) -> Select<R>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
    {
        Select::<R>::find_related_to::<Self::Entity>(self)
    }

    /// Builds a query for the rows reached from this model through the
    /// chain `link`.
    fn find_linked<L>(&self, link: L) -> Select<L::ToEntity>
    where
        L: Linked<FromEntity = Self::Entity>,
    {
        Select::<L::ToEntity>::find_linked_to(&link, self)
    }

    /// Deletes the row this model was read from, running the active model's
    /// [`ActiveModelBehavior`] hooks.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`ActiveModelTrait::delete`].
    async fn delete<C>(self, db: &C) -> Result<DeleteResult>
    where
        C: ConnectionTrait,
        Self: IntoActiveModel<<Self::Entity as EntityTrait>::ActiveModel>,
        <Self::Entity as EntityTrait>::ActiveModel: ActiveModelBehavior,
    {
        self.into_active_model().delete(db).await
    }
}

/// A type that can be built from a result row.
///
/// Derived by `DeriveEntityModel` for models and by `FromQueryResult` for
/// custom projection structs; implemented here for tuples, read by
/// position, and for a JSON object, behind the `with-json` feature.
pub trait FromQueryResult: Sized + Send + Sync {
    /// Builds a value from `row`, reading columns named `{prefix}{column}`.
    ///
    /// The prefix is empty for plain selects and `A_` / `B_` for the two
    /// sides of a `find_also_related` query.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when a required column is missing from the
    /// row or cannot be decoded as the field type.
    fn from_query_result(row: &Row, prefix: &str) -> Result<Self>;

    /// Like [`from_query_result`](Self::from_query_result), but returns
    /// `Ok(None)` when every column carrying `prefix` is `NULL`.
    ///
    /// This is how the optional side of a `LEFT JOIN` is detected: a row
    /// without a match has all of its `B_` columns `NULL`, which would
    /// otherwise fail to decode into non-nullable fields.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Driver`] when a column is present but cannot be decoded.
    fn from_query_result_optional(row: &Row, prefix: &str) -> Result<Option<Self>> {
        let all_null = row
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .all(|(_, v)| matches!(v, Value::Null));
        if all_null {
            Ok(None)
        } else {
            Self::from_query_result(row, prefix).map(Some)
        }
    }
}

impl FromQueryResult for Row {
    fn from_query_result(row: &Row, _prefix: &str) -> Result<Self> {
        Ok(row.clone())
    }
}

/// Implements [`FromQueryResult`] for tuples, decoding the select list by
/// position and ignoring the prefix.
macro_rules! tuple_from_query_result {
    ($(($($t:ident $i:tt),+));* $(;)?) => {$(
        impl<$($t: FromValue + Send + Sync),+> FromQueryResult for ($($t,)+) {
            fn from_query_result(row: &Row, _prefix: &str) -> Result<Self> {
                Ok(($(row.get::<$t>($i)?,)+))
            }
        }
    )*};
}

tuple_from_query_result! {
    (A 0);
    (A 0, B 1);
    (A 0, B 1, C 2);
    (A 0, B 1, C 2, D 3);
    (A 0, B 1, C 2, D 3, E 4);
    (A 0, B 1, C 2, D 3, E 4, F 5);
}

/// Decodes a row into a JSON object keyed by the column names without
/// `prefix`.
///
/// Integers and reals become numbers, text a string, `NULL` null, and a
/// blob an array of byte values, since JSON has no binary type.
#[cfg(feature = "with-json")]
#[cfg_attr(docsrs, doc(cfg(feature = "with-json")))]
impl FromQueryResult for serde_json::Value {
    fn from_query_result(row: &Row, prefix: &str) -> Result<Self> {
        let mut object = serde_json::Map::new();
        for (name, value) in row.iter() {
            let Some(name) = name.strip_prefix(prefix) else {
                continue;
            };
            let json = match value {
                Value::Null => serde_json::Value::Null,
                Value::Integer(n) => serde_json::Value::from(*n),
                Value::Real(f) => serde_json::Value::from(*f),
                Value::Text(s) => serde_json::Value::String(s.clone()),
                Value::Blob(b) => serde_json::Value::Array(
                    b.iter()
                        .map(|byte| serde_json::Value::from(*byte))
                        .collect(),
                ),
            };
            object.insert(name.to_owned(), json);
        }
        Ok(serde_json::Value::Object(object))
    }
}

/// Reads one column of `row` into a field type.
///
/// Used by generated `from_query_result` impls; the column is looked up as
/// `{prefix}{column}` so the same impl serves plain and aliased selects.
///
/// # Errors
///
/// Returns [`DbErr::Driver`] when the column is missing from the row or
/// cannot be decoded as `T`.
pub fn get_field<T: FromValue>(row: &Row, prefix: &str, column: &str) -> Result<T> {
    let name = if prefix.is_empty() {
        column.to_owned()
    } else {
        format!("{prefix}{column}")
    };
    row.get::<T>(name.as_str()).map_err(DbErr::from)
}
