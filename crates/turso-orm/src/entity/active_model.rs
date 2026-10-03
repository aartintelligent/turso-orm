//! Active models, the mutable change-tracking form of a model, modeled by [`ActiveModelTrait`].
//!
//! A model is a snapshot; an active model is the same set of attributes with
//! each one wrapped in an [`ActiveValue`] that records whether it should be
//! written. That distinction is what lets `INSERT` send only the attributes
//! the caller set (so database defaults apply to the rest), lets `UPDATE`
//! touch only the attributes that changed, and lets [`ActiveModelTrait::save`]
//! decide between insert and update by looking at the primary key alone.
//!
//! This module owns the attribute state machine, the write entry points
//! (`insert`, `update`, `save`, `delete`) and the [`ActiveModelBehavior`]
//! hooks around them. It does not render SQL — the `crate::query` builders
//! do — and [`ActiveModelTrait::set`] returns a `Result` instead of
//! panicking so that a value of the wrong type surfaces as [`DbErr::Type`].
//!
//! Three conversions complete the picture. [`IntoActiveModel`] turns a
//! model, or a struct derived with `DeriveIntoActiveModel`, into an active
//! model; [`IntoActiveValue`] is the per-attribute rule that derive applies,
//! where an `Option` field means "set only when `Some`"; and
//! [`TryIntoModel`] goes back from an active model whose every attribute
//! carries a value to the plain model. Behind the `with-json` feature,
//! [`ActiveModelTrait::set_from_json`] fills attributes from a JSON object
//! keyed by column name, which is what a request body usually is.
//!
//! `decode_field` is exposed through the crate's `__private` module for
//! generated code only.

use async_trait::async_trait;
use turso_orm_driver::ConnectionTrait;
use turso_sql::Value;

use super::base_entity::EntityTrait;
use crate::query::{DeleteResult, Insert, UpdateOne};
use crate::{DbErr, Result};

/// The state of one attribute of an active model.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ActiveValue<T> {
    /// A new value that will be written on the next insert or update.
    Set(T),
    /// The value currently stored; it is read back but never written.
    Unchanged(T),
    /// No value; the database default applies on insert.
    #[default]
    NotSet,
}

/// Builds an [`ActiveValue::Set`].
///
/// Named in `UpperCamelCase` so that `Set(v)` reads like the variant it
/// wraps in struct literals.
#[allow(
    non_snake_case,
    reason = "mirrors the variant name for struct literals"
)]
pub fn Set<T>(value: T) -> ActiveValue<T> {
    ActiveValue::Set(value)
}

/// Builds an [`ActiveValue::Unchanged`].
///
/// Named in `UpperCamelCase` for the same reason as [`Set`].
#[allow(
    non_snake_case,
    reason = "mirrors the variant name for struct literals"
)]
pub fn Unchanged<T>(value: T) -> ActiveValue<T> {
    ActiveValue::Unchanged(value)
}

/// The [`ActiveValue::NotSet`] constant, usable in struct literals.
#[allow(
    non_upper_case_globals,
    reason = "mirrors the variant name for struct literals"
)]
pub const NotSet: ActiveValue<()> = ActiveValue::NotSet;

impl<T> ActiveValue<T> {
    /// Whether the attribute is `Set`.
    pub fn is_set(&self) -> bool {
        matches!(self, ActiveValue::Set(_))
    }

    /// Whether the attribute is `Unchanged`.
    pub fn is_unchanged(&self) -> bool {
        matches!(self, ActiveValue::Unchanged(_))
    }

    /// Whether the attribute is `NotSet`.
    pub fn is_not_set(&self) -> bool {
        matches!(self, ActiveValue::NotSet)
    }

    /// The value, whether `Set` or `Unchanged`.
    pub fn as_ref(&self) -> Option<&T> {
        match self {
            ActiveValue::Set(v) | ActiveValue::Unchanged(v) => Some(v),
            ActiveValue::NotSet => None,
        }
    }

    /// Takes the value out, leaving `NotSet` behind.
    pub fn take(&mut self) -> Option<T> {
        match std::mem::replace(self, ActiveValue::NotSet) {
            ActiveValue::Set(v) | ActiveValue::Unchanged(v) => Some(v),
            ActiveValue::NotSet => None,
        }
    }

    /// Consumes the attribute and returns its value, whether `Set` or `Unchanged`.
    pub fn into_value(self) -> Option<T> {
        match self {
            ActiveValue::Set(v) | ActiveValue::Unchanged(v) => Some(v),
            ActiveValue::NotSet => None,
        }
    }

    /// Marks the attribute `Unchanged`, keeping its value; used after a successful write.
    pub fn reset(&mut self) {
        // The state is moved out exactly once: a second `mem::replace` would
        // read the `NotSet` placeholder and lose an `Unchanged` value.
        *self = match std::mem::replace(self, ActiveValue::NotSet) {
            ActiveValue::Set(v) | ActiveValue::Unchanged(v) => ActiveValue::Unchanged(v),
            ActiveValue::NotSet => ActiveValue::NotSet,
        };
    }

    /// Maps the inner value, preserving the attribute state.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ActiveValue<U> {
        match self {
            ActiveValue::Set(v) => ActiveValue::Set(f(v)),
            ActiveValue::Unchanged(v) => ActiveValue::Unchanged(f(v)),
            ActiveValue::NotSet => ActiveValue::NotSet,
        }
    }
}

impl<T: PartialEq> ActiveValue<T> {
    /// Sets `value` unless it equals the current `Unchanged` value, in which
    /// case the attribute stays `Unchanged` and is not written back.
    pub fn set_if_not_equals(&mut self, value: T) {
        match self {
            ActiveValue::Unchanged(current) if *current == value => {}
            _ => *self = ActiveValue::Set(value),
        }
    }
}

impl<T> From<T> for ActiveValue<T> {
    fn from(value: T) -> Self {
        ActiveValue::Set(value)
    }
}

/// Conversion into an entity's active model, from the model or as identity.
pub trait IntoActiveModel<A: ActiveModelTrait> {
    /// Converts into the active model.
    fn into_active_model(self) -> A;
}

impl<A: ActiveModelTrait> IntoActiveModel<A> for A {
    fn into_active_model(self) -> A {
        self
    }
}

/// Conversion of one field of a plain struct into an [`ActiveValue`], as
/// applied by `DeriveIntoActiveModel`.
///
/// A plain value becomes `Set`. An `Option` wrapping the attribute type
/// becomes `Set` when `Some` and `NotSet` when `None`, so that an optional
/// field of a request leaves the column alone; an `Option` that *is* the
/// attribute type — a nullable column — is always `Set`, `None` included,
/// and `Option<Option<T>>` is the form that leaves a nullable column alone.
/// Rust picks the right rule from the attribute type the active model
/// declares.
pub trait IntoActiveValue<T> {
    /// Converts into the attribute state.
    fn into_active_value(self) -> ActiveValue<T>;
}

impl<T: crate::types::TursoType> IntoActiveValue<T> for T {
    fn into_active_value(self) -> ActiveValue<T> {
        ActiveValue::Set(self)
    }
}

impl<T: crate::types::TursoType> IntoActiveValue<T> for Option<T> {
    fn into_active_value(self) -> ActiveValue<T> {
        match self {
            Some(v) => ActiveValue::Set(v),
            None => ActiveValue::NotSet,
        }
    }
}

/// Conversion of an active model back into the plain model.
///
/// Implemented by `DeriveEntityModel` on the generated `ActiveModel`.
pub trait TryIntoModel<M> {
    /// Builds the model from the attribute values.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::AttrNotSet`] naming the first attribute that is
    /// `NotSet`.
    fn try_into_model(self) -> Result<M>;
}

/// The mutable form of an entity, derived by `DeriveEntityModel`.
#[async_trait]
pub trait ActiveModelTrait: Clone + Send + Sync + std::fmt::Debug + Default {
    /// The entity this active model belongs to.
    type Entity: EntityTrait<ActiveModel = Self>;

    /// Reads an attribute as a SQL value together with its state.
    fn get(&self, column: <Self::Entity as EntityTrait>::Column) -> ActiveValue<Value>;

    /// Sets an attribute from a SQL value, marking it `Set`.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Type`] when the value cannot be decoded as the field type.
    fn set(&mut self, column: <Self::Entity as EntityTrait>::Column, value: Value) -> Result<()>;

    /// Marks an attribute `NotSet`.
    fn not_set(&mut self, column: <Self::Entity as EntityTrait>::Column);

    /// Whether an attribute is `NotSet`.
    fn is_not_set(&self, column: <Self::Entity as EntityTrait>::Column) -> bool;

    /// Marks an attribute `Unchanged`.
    fn reset(&mut self, column: <Self::Entity as EntityTrait>::Column);

    /// Marks every attribute `Unchanged`.
    #[must_use]
    fn reset_all(mut self) -> Self {
        for c in <<Self::Entity as EntityTrait>::Column as super::Iterable>::iter() {
            self.reset(c);
        }
        self
    }

    /// Whether any attribute is `Set`.
    fn is_changed(&self) -> bool {
        <<Self::Entity as EntityTrait>::Column as super::Iterable>::iter()
            .any(|c| self.get(c).is_set())
    }

    /// Sets every attribute named by a key of the JSON object `json`,
    /// matching keys against column names.
    ///
    /// Keys that are not columns are ignored. Numbers, strings, booleans
    /// and `null` map onto the storage classes; an array or an object is
    /// stored as its JSON text, which is how a JSON column expects it.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::Json`] when `json` is not an object; [`DbErr::Type`]
    /// when a value cannot be decoded as the attribute type.
    #[cfg(feature = "with-json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "with-json")))]
    fn set_from_json(&mut self, json: serde_json::Value) -> Result<()> {
        let serde_json::Value::Object(map) = json else {
            return Err(DbErr::Json("expected a JSON object".into()));
        };
        for c in <<Self::Entity as EntityTrait>::Column as super::Iterable>::iter() {
            if let Some(v) =
                map.get(<<Self::Entity as EntityTrait>::Column as super::IdenStatic>::as_str(&c))
            {
                self.set(c, json_to_value(v))?;
            }
        }
        Ok(())
    }

    /// Builds an active model with every attribute named in `json` set and
    /// the others `NotSet`; see [`set_from_json`](Self::set_from_json).
    ///
    /// # Errors
    ///
    /// Returns the errors of [`set_from_json`](Self::set_from_json).
    #[cfg(feature = "with-json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "with-json")))]
    fn from_json(json: serde_json::Value) -> Result<Self> {
        let mut am = Self::default();
        am.set_from_json(json)?;
        Ok(am)
    }

    /// The primary key values, or `None` when any key attribute is `NotSet`.
    fn get_primary_key_value(&self) -> Option<Vec<Value>> {
        use super::primary_key::PrimaryKeyToColumn;
        let mut values = Vec::new();
        for pk in <<Self::Entity as EntityTrait>::PrimaryKey as super::Iterable>::iter() {
            let value = self.get(pk.into_column()).into_value()?;
            values.push(value);
        }
        Some(values)
    }

    /// Inserts the active model and returns the stored model via `INSERT ... RETURNING *`.
    ///
    /// The [`ActiveModelBehavior`] hooks run around the statement.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::RecordNotInserted`] when the statement inserted
    /// nothing; [`DbErr::Driver`] when the statement or the decoding of the
    /// returned row fails; any error raised by the hooks.
    async fn insert<C: ConnectionTrait>(
        self,
        db: &C,
    ) -> Result<<Self::Entity as EntityTrait>::Model>
    where
        Self: ActiveModelBehavior,
    {
        let am = <Self as ActiveModelBehavior>::before_save(self, db, true).await?;
        let model = Insert::<Self>::one(am).exec_with_returning(db).await?;
        <Self as ActiveModelBehavior>::after_save(model, db, true).await
    }

    /// Updates the row matched by the primary key and returns the stored model.
    ///
    /// Only `Set` attributes are written; with none, the row is simply
    /// fetched. The [`ActiveModelBehavior`] hooks run around the statement.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::PrimaryKeyNotSet`] when a key attribute is `NotSet`;
    /// [`DbErr::RecordNotUpdated`] when no row matches the key;
    /// [`DbErr::Driver`] when the statement or the decoding of the returned
    /// row fails; any error raised by the hooks.
    async fn update<C: ConnectionTrait>(
        self,
        db: &C,
    ) -> Result<<Self::Entity as EntityTrait>::Model>
    where
        Self: ActiveModelBehavior,
    {
        let am = <Self as ActiveModelBehavior>::before_save(self, db, false).await?;
        let model = UpdateOne::new(am).exec(db).await?;
        <Self as ActiveModelBehavior>::after_save(model, db, false).await
    }

    /// Inserts when the primary key is `NotSet` or `Set`, updates when it is `Unchanged`.
    ///
    /// Returns the active model with every attribute `Unchanged`, so that it
    /// can be modified and saved again.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`insert`](Self::insert) or
    /// [`update`](Self::update), whichever ran.
    async fn save<C: ConnectionTrait>(self, db: &C) -> Result<Self>
    where
        Self: ActiveModelBehavior,
        <Self::Entity as EntityTrait>::Model: IntoActiveModel<Self>,
    {
        let model = if self.is_update() {
            self.update(db).await?
        } else {
            self.insert(db).await?
        };
        Ok(model.into_active_model())
    }

    /// Whether [`save`](Self::save) would update: every primary-key attribute is `Unchanged`.
    fn is_update(&self) -> bool {
        use super::primary_key::PrimaryKeyToColumn;
        let mut keys = <<Self::Entity as EntityTrait>::PrimaryKey as super::Iterable>::iter();
        // An entity without key columns can never be updated by key, and
        // `all` on an empty iterator would wrongly say yes.
        let Some(first) = keys.next() else {
            return false;
        };
        std::iter::once(first)
            .chain(keys)
            .all(|pk| self.get(pk.into_column()).is_unchanged())
    }

    /// Deletes the row matched by the primary key.
    ///
    /// The [`ActiveModelBehavior`] hooks run around the statement.
    ///
    /// # Errors
    ///
    /// Returns [`DbErr::PrimaryKeyNotSet`] when a key attribute is `NotSet`;
    /// [`DbErr::Driver`] when the statement fails; any error raised by the
    /// hooks.
    async fn delete<C: ConnectionTrait>(self, db: &C) -> Result<DeleteResult>
    where
        Self: ActiveModelBehavior,
    {
        let am = <Self as ActiveModelBehavior>::before_delete(self, db).await?;
        let result = crate::query::DeleteOne::new(am.clone()).exec(db).await?;
        <Self as ActiveModelBehavior>::after_delete(am, db).await?;
        Ok(result)
    }
}

/// Hooks around writes, with no-op defaults.
///
/// Implement with an empty body to accept the defaults:
///
/// ```ignore
/// impl ActiveModelBehavior for ActiveModel {}
/// ```
#[async_trait]
pub trait ActiveModelBehavior: ActiveModelTrait {
    /// Called before `insert` (`insert == true`) or `update`; may rewrite the active model.
    ///
    /// # Errors
    ///
    /// Returns whatever the implementation chooses to fail with; the
    /// default never fails.
    async fn before_save<C: ConnectionTrait>(self, _db: &C, _insert: bool) -> Result<Self> {
        Ok(self)
    }

    /// Called after a successful `insert` or `update` with the stored model.
    ///
    /// # Errors
    ///
    /// Returns whatever the implementation chooses to fail with; the
    /// default never fails.
    async fn after_save<C: ConnectionTrait>(
        model: <Self::Entity as EntityTrait>::Model,
        _db: &C,
        _insert: bool,
    ) -> Result<<Self::Entity as EntityTrait>::Model> {
        Ok(model)
    }

    /// Called before `delete`; may rewrite the active model.
    ///
    /// # Errors
    ///
    /// Returns whatever the implementation chooses to fail with; the
    /// default never fails.
    async fn before_delete<C: ConnectionTrait>(self, _db: &C) -> Result<Self> {
        Ok(self)
    }

    /// Called after a successful `delete`.
    ///
    /// # Errors
    ///
    /// Returns whatever the implementation chooses to fail with; the
    /// default never fails.
    async fn after_delete<C: ConnectionTrait>(self, _db: &C) -> Result<Self> {
        Ok(self)
    }
}

/// Flattens a JSON value onto a storage class for [`ActiveModelTrait::set_from_json`].
///
/// Integers that fit `i64` stay integers, other numbers become reals, and
/// compound values are stored as their JSON text.
#[cfg(feature = "with-json")]
fn json_to_value(json: &serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Integer(i64::from(*b)),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Value::Integer)
            .or_else(|| n.as_f64().map(Value::Real))
            .unwrap_or(Value::Null),
        serde_json::Value::String(s) => Value::Text(s.clone()),
        compound => Value::Text(compound.to_string()),
    }
}

/// Decodes a SQL value into a field type for generated `set` impls.
///
/// The driver's `FromValue` decoders take the builder's [`Value`] directly,
/// so this only maps the error type.
///
/// # Errors
///
/// Returns [`DbErr::Type`] when the value cannot be decoded as `T`.
pub fn decode_field<T: turso_orm_driver::FromValue>(column: &str, value: Value) -> Result<T> {
    T::from_value(value, column).map_err(|e| DbErr::Type(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::ActiveValue;

    /// `reset` turns a `Set` value into `Unchanged`, keeping the value.
    #[test]
    fn reset_set_becomes_unchanged() {
        let mut value = ActiveValue::Set(42);
        value.reset();
        assert_eq!(value, ActiveValue::Unchanged(42));
    }

    /// `reset` leaves an `Unchanged` value as it is.
    #[test]
    fn reset_unchanged_stays_unchanged() {
        let mut value = ActiveValue::Unchanged(42);
        value.reset();
        assert_eq!(value, ActiveValue::Unchanged(42));
    }

    /// `reset` leaves a `NotSet` attribute `NotSet`.
    #[test]
    fn reset_not_set_stays_not_set() {
        let mut value = ActiveValue::<i32>::NotSet;
        value.reset();
        assert_eq!(value, ActiveValue::NotSet);
    }

    /// `reset` is idempotent on every state.
    #[test]
    fn reset_is_idempotent() {
        for start in [
            ActiveValue::Set(1),
            ActiveValue::Unchanged(1),
            ActiveValue::NotSet,
        ] {
            let mut once = start.clone();
            once.reset();
            let mut twice = once.clone();
            twice.reset();
            assert_eq!(once, twice);
        }
    }
}
