//! The `llm_model` entity: served by a provider, configured on agents.

use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

/// A row of the `llm_model` table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel)]
#[turso(table_name = "llm_model")]
pub struct Model {
    /// Auto-incremented key.
    #[turso(primary_key)]
    pub id: i32,
    /// The model name, as the provider's API knows it.
    pub name: String,
    /// Price per million input tokens; use `with-rust_decimal` for money in a real app.
    pub input_price: f64,
    /// Whether the model can call tools.
    pub supports_tools: bool,
    /// Free-form capabilities stored as JSON: context window, modalities, and so on.
    pub capabilities: Option<Json>,
    /// The serving provider; indexed because every lookup goes through it.
    #[turso(indexed)]
    pub provider_id: i32,
}

/// The relations of `llm_model`.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    /// The serving provider; deleting the provider deletes its models.
    #[turso(
        belongs_to = "super::llm_provider::Entity",
        from = "Column::ProviderId",
        to = "super::llm_provider::Column::Id",
        on_delete = "Cascade"
    )]
    LlmProvider,
    /// One model, many agents configured on it.
    #[turso(has_many = "super::agent::Entity")]
    Agent,
}

impl ActiveModelBehavior for ActiveModel {}
