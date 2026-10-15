//! The `tool` entity: a capability an agent can call, shared by many threads.

use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

/// A row of the `tool` table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel)]
#[turso(table_name = "tool")]
pub struct Model {
    /// Auto-incremented key.
    #[turso(primary_key)]
    pub id: i32,
    /// The tool name, unique across the table, as the model calls it.
    #[turso(unique)]
    pub name: String,
    /// What the tool does, as shown to the model.
    pub description: Option<String>,
}

/// The relations of `tool`: the junction rows, and the threads through them.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    #[turso(has_many = "super::thread_tool::Entity")]
    ThreadTool,
    #[turso(has_many = "super::thread::Entity", via = "super::thread_tool::Entity")]
    Threads,
}

impl ActiveModelBehavior for ActiveModel {}
