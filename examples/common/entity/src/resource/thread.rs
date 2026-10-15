//! The `thread` entity: a conversation an agent owns, optionally reviewed by
//! another agent and optionally forked from an earlier thread.

use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

// --8<-- [start:active_enum]
/// The state of a thread, stored as text.
///
/// Without `string_value`, a variant is stored as its `snake_case` name; the
/// `serde` attribute keeps the JSON value equal to the stored one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, DeriveActiveEnum)]
#[serde(rename_all = "snake_case")]
#[turso(rs_type = "String")]
pub enum Status {
    Open,
    #[turso(string_value = "done")]
    #[serde(rename = "done")]
    Completed,
}
// --8<-- [end:active_enum]

/// A row of the `thread` table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, DeriveEntityModel)]
#[turso(table_name = "thread")]
pub struct Model {
    /// Auto-incremented key.
    #[turso(primary_key)]
    pub id: i32,
    /// The owning agent; indexed because every listing goes through it.
    #[turso(indexed)]
    pub agent_id: i32,
    /// A second agent reviewing the thread, if any.
    pub reviewer_agent_id: Option<i32>,
    /// The thread this one was forked from, if any.
    pub parent_id: Option<i32>,
    /// The title.
    pub title: String,
    /// The state; new threads are open.
    #[turso(default_value = "open")]
    pub status: Status,
}

// --8<-- [start:relations]
/// The relations of `thread`.
///
/// `Agent` and `Reviewer` both point at `agent`: the first one provides
/// `Related<agent::Entity>`, the second is used through its `def()`.
/// `Parent` is the self-reference and `Children` its inferred reverse.
/// `Tools` goes through the `thread_tool` junction.
#[derive(Copy, Clone, Debug, DeriveRelation)]
pub enum Relation {
    #[turso(
        belongs_to = "super::agent::Entity",
        from = "Column::AgentId",
        to = "super::agent::Column::Id",
        on_delete = "Cascade"
    )]
    Agent,
    #[turso(
        belongs_to = "super::agent::Entity",
        from = "Column::ReviewerAgentId",
        to = "super::agent::Column::Id",
        on_delete = "SetNull"
    )]
    Reviewer,
    #[turso(
        belongs_to = "Entity",
        from = "Column::ParentId",
        to = "Column::Id",
        on_delete = "Cascade"
    )]
    Parent,
    #[turso(has_many = "Entity")]
    Children,
    #[turso(has_many = "super::thread_record::Entity")]
    ThreadRecord,
    #[turso(has_many = "super::thread_tool::Entity")]
    ThreadTool,
    #[turso(has_many = "super::tool::Entity", via = "super::thread_tool::Entity")]
    Tools,
}
// --8<-- [end:relations]

impl ActiveModelBehavior for ActiveModel {}
