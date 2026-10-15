//! The `llm_model` entity: the root of the schema, configured on agents.

use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

/// A row of the `llm_model` table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel)]
#[turso(table_name = "llm_model")]
pub struct Model {
    /// Auto-incremented key.
    #[turso(primary_key)]
    pub id: i32,
    /// The model name, as its API knows it.
    pub name: String,
    /// Price per million input tokens; use `with-rust_decimal` for money in a real app.
    pub input_price: f64,
    /// Whether the model can call tools.
    pub supports_tools: bool,
    /// Free-form capabilities stored as JSON: context window, modalities, and so on.
    pub capabilities: Option<Json>,
}

/// The relations of `llm_model`.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    /// One model, many agents; the join columns come from `agent`'s `belongs_to`.
    #[turso(has_many = "super::agent::Entity")]
    Agent,
}

impl ActiveModelBehavior for ActiveModel {}
