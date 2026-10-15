//! The `llm_provider` entity: the root of the schema, serving models.

use turso_orm::prelude::*;

/// A row of the `llm_provider` table.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[turso(table_name = "llm_provider")]
pub struct Model {
    /// Auto-incremented key.
    #[turso(primary_key)]
    pub id: i32,
    /// The provider name, unique across the table.
    #[turso(unique)]
    pub name: String,
    /// The base URL of its API.
    pub base_url: String,
}

/// The relations of `llm_provider`.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    /// One provider, many models; the join columns come from `llm_model`'s `belongs_to`.
    #[turso(has_many = "super::llm_model::Entity")]
    LlmModel,
}

impl ActiveModelBehavior for ActiveModel {}
