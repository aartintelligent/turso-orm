//! The `agent` entity: a named configuration on a model, owning threads.

use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

/// A row of the `agent` table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel)]
#[turso(table_name = "agent")]
pub struct Model {
    /// Auto-incremented key.
    #[turso(primary_key)]
    pub id: i32,
    /// The agent name, unique across the table.
    #[turso(unique)]
    pub name: String,
    /// The system prompt, when the agent has one of its own.
    pub system_prompt: Option<String>,
    /// The model the agent runs on, if one is configured; detaching keeps the agent.
    pub model_id: Option<i32>,
}

/// The relations of `agent`.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    /// The configured model; deleting the model detaches the agent.
    #[turso(
        belongs_to = "super::llm_model::Entity",
        from = "Column::ModelId",
        to = "super::llm_model::Column::Id",
        on_delete = "SetNull"
    )]
    LlmModel,
    /// The threads the agent owns; the columns come from the first relation
    /// `thread` declares back to `agent`, the owner, not the reviewer.
    #[turso(has_many = "super::thread::Entity")]
    Thread,
}

impl ActiveModelBehavior for ActiveModel {}
