//! The entity layer: entities, models, active models, columns, primary keys and relations.
//!
//! An entity module written by hand or expanded by `DeriveEntityModel`
//! consists of a unit struct `Entity`, a plain `Model` struct holding one
//! row, a `Column` enum, a `PrimaryKey` enum, a change-tracking `ActiveModel`
//! and a `Relation` enum. This module owns the traits each of those types
//! implements and the glue between them; it does not own SQL rendering
//! (that is `turso_sql`) nor execution (that is the driver).
//!
//! - [`EntityTrait`] ties the pieces together and is the entry point for
//!   queries (`Entity::find()`, `Entity::insert(..)`).
//! - [`ModelTrait`] and [`FromQueryResult`] read rows into structs.
//! - [`ActiveModelTrait`] and [`ActiveValue`] track which attributes to write.
//! - [`ColumnTrait`] provides the condition builders on column enums.
//! - [`PrimaryKeyTrait`] and [`PrimaryKeyToColumn`] describe the key.
//! - [`RelationTrait`], [`Related`], [`RelationDef`] and [`Linked`] describe
//!   joins, junction tables, multi-hop chains and the foreign keys they
//!   imply.
//! - [`LoaderTrait`] batch-loads related rows for a list of models.
//! - [`ActiveEnum`] maps a Rust enum onto a text or integer column.
//! - [`PartialModelTrait`] selects and decodes a subset of columns.
//! - [`Schema`] turns an entity into `CREATE TABLE` and `CREATE INDEX`
//!   statements.
//!
//! The `active_model`, `iden`, `model` and `relation` submodules are
//! `pub(crate)` because the crate root re-exports helpers from them through
//! `__private` for the benefit of generated code, or because the query
//! builders share helpers with them.

pub(crate) mod active_enum;
pub(crate) mod active_model;
mod base_entity;
mod column;
pub(crate) mod iden;
mod loader;
pub(crate) mod model;
mod partial_model;
mod primary_key;
pub(crate) mod relation;
mod schema;

pub use active_enum::ActiveEnum;
pub use active_model::{
    ActiveModelBehavior, ActiveModelTrait, ActiveValue, IntoActiveModel, IntoActiveValue, NotSet,
    Set, TryIntoModel, Unchanged,
};
pub use base_entity::EntityTrait;
pub use column::{ColumnDef, ColumnTrait};
pub use iden::{IdenStatic, Iterable};
pub use loader::LoaderTrait;
pub use model::{FromQueryResult, ModelTrait};
pub use partial_model::PartialModelTrait;
pub use primary_key::{PrimaryKeyToColumn, PrimaryKeyTrait};
pub use relation::{Linked, Related, RelationBuilder, RelationDef, RelationTrait, RelationType};
pub use schema::Schema;
