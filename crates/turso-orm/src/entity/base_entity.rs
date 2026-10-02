//! The entity trait, modeled by [`EntityTrait`].
//!
//! `EntityTrait` is implemented on the unit struct `Entity` of each entity
//! module and is the hub that names the model, column, key, active model and
//! relation types belonging to one table. All of its methods are
//! constructors for the query builders in `crate::query`; the builders do
//! the actual work so that this trait stays a thin, stable surface that the
//! derive macro can implement with a handful of associated types.

use super::active_model::ActiveModelTrait;
use super::column::ColumnTrait;
use super::iden::IdenStatic;
use super::model::ModelTrait;
use super::primary_key::{PrimaryKeyToColumn, PrimaryKeyTrait};
use super::relation::{RelationBuilder, RelationTrait, RelationType};
use crate::query::{DeleteMany, DeleteOne, Insert, InsertMany, Select, UpdateMany, UpdateOne};
use crate::types::IntoValueTuple;

/// An entity: a table together with its model, columns, key and relations.
///
/// Derived by `DeriveEntityModel`; the unit struct `Entity` implements it.
pub trait EntityTrait: IdenStatic + Default {
    /// The model struct.
    type Model: ModelTrait<Entity = Self> + super::FromQueryResult;
    /// The column enum.
    type Column: ColumnTrait;
    /// The primary key enum.
    type PrimaryKey: PrimaryKeyTrait + PrimaryKeyToColumn<Column = Self::Column>;
    /// The active model struct.
    type ActiveModel: ActiveModelTrait<Entity = Self>;
    /// The relation enum.
    type Relation: RelationTrait;

    /// The table name.
    const TABLE_NAME: &'static str;

    /// The table name, as a method for contexts that cannot name the constant.
    fn table_name() -> &'static str {
        Self::TABLE_NAME
    }

    /// Starts a `belongs_to` relation, in which `Self` holds the foreign key.
    fn belongs_to<R: EntityTrait>(_: R) -> RelationBuilder<Self, R> {
        RelationBuilder::new(RelationType::HasOne, true)
    }

    /// Starts a `has_one` relation, in which `R` holds the foreign key.
    fn has_one<R: EntityTrait>(_: R) -> RelationBuilder<Self, R> {
        RelationBuilder::new(RelationType::HasOne, false)
    }

    /// Starts a `has_many` relation, in which `R` holds the foreign key.
    fn has_many<R: EntityTrait>(_: R) -> RelationBuilder<Self, R> {
        RelationBuilder::new(RelationType::HasMany, false)
    }

    /// Builds `SELECT * FROM table`.
    fn find() -> Select<Self> {
        Select::new()
    }

    /// Builds `SELECT * FROM table WHERE pk = id`.
    ///
    /// Composite keys are passed as a tuple in key-column order.
    fn find_by_id<T>(id: T) -> Select<Self>
    where
        T: Into<<Self::PrimaryKey as PrimaryKeyTrait>::ValueType>,
    {
        Select::new().filter_by_pk(id.into().into_value_tuple())
    }

    /// Builds an `INSERT` of one active model.
    fn insert<A: ActiveModelTrait<Entity = Self>>(model: A) -> Insert<A> {
        Insert::one(model)
    }

    /// Builds a single `INSERT` of several active models.
    fn insert_many<A: ActiveModelTrait<Entity = Self>, I: IntoIterator<Item = A>>(
        models: I,
    ) -> InsertMany<A> {
        InsertMany::many(models)
    }

    /// Builds an `UPDATE` of one active model, matched by primary key.
    fn update<A: ActiveModelTrait<Entity = Self>>(model: A) -> UpdateOne<A> {
        UpdateOne::new(model)
    }

    /// Builds an `UPDATE` over many rows.
    fn update_many() -> UpdateMany<Self> {
        UpdateMany::new()
    }

    /// Builds a `DELETE` of one active model, matched by primary key.
    fn delete<A: ActiveModelTrait<Entity = Self>>(model: A) -> DeleteOne<A> {
        DeleteOne::new(model)
    }

    /// Builds a `DELETE` over many rows.
    fn delete_many() -> DeleteMany<Self> {
        DeleteMany::new()
    }

    /// Builds a `DELETE` matched by primary key.
    fn delete_by_id<T>(id: T) -> DeleteMany<Self>
    where
        T: Into<<Self::PrimaryKey as PrimaryKeyTrait>::ValueType>,
    {
        DeleteMany::new().filter_by_pk(id.into().into_value_tuple())
    }
}
