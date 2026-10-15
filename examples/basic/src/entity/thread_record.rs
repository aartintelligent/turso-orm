//! The `thread_record` entity: one step of a thread, a prompt, an answer or a tool call.

use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

/// Who produced a record, stored as text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveActiveEnum)]
#[serde(rename_all = "snake_case")]
#[turso(rs_type = "String")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A row of the `thread_record` table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel)]
#[turso(table_name = "thread_record")]
pub struct Model {
    /// Auto-incremented key.
    #[turso(primary_key)]
    pub id: i32,
    /// The thread the record belongs to; indexed because a transcript is read through it.
    #[turso(indexed)]
    pub thread_id: i32,
    /// The position in the thread, from one.
    pub seq: i32,
    /// Who produced the record.
    pub role: Role,
    /// The text, absent for a record that only carries tool calls.
    pub content: Option<String>,
    /// The tool calls the model asked for, as the API returned them.
    pub tool_calls: Option<Json>,
    /// Tokens of the prompt that produced an assistant record.
    pub prompt_tokens: Option<i32>,
    /// Tokens of the completion of an assistant record.
    pub completion_tokens: Option<i32>,
}

/// The relations of `thread_record`.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    /// The thread; deleting the thread deletes its records.
    #[turso(
        belongs_to = "super::thread::Entity",
        from = "Column::ThreadId",
        to = "super::thread::Column::Id",
        on_delete = "Cascade"
    )]
    Thread,
}

impl ActiveModelBehavior for ActiveModel {}
