//! Bodies, parameters and responses of the threads.

use crate::entity::{thread, thread_record, tool};
use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

// --8<-- [start:into_active_model]
/// The body of `POST /threads`: a subset of the columns, with an optional
/// reviewer that is only set when the client sends one.
#[derive(Debug, Deserialize, DeriveIntoActiveModel)]
#[turso(active_model = "thread::ActiveModel")]
pub struct OpenThread {
    pub agent_id: i32,
    pub title: String,
    pub reviewer_agent_id: Option<i32>,
}
// --8<-- [end:into_active_model]

/// The body of `POST /threads/with-tools`: a thread and the tools it starts with.
#[derive(Debug, Deserialize)]
pub struct OpenWithTools {
    pub agent_id: i32,
    pub title: String,
    pub tools: Vec<String>,
}

/// The body of `POST /threads/{id}/fork`.
#[derive(Debug, Deserialize)]
pub struct Fork {
    pub agent_id: i32,
}

/// `DELETE /threads` parameters.
#[derive(Debug, Deserialize)]
pub struct Purge {
    /// Delete the threads whose title starts with this.
    pub prefix: String,
}

// --8<-- [start:partial_model]
/// A projection of `thread`: two columns under their own names and one
/// expression, selected and decoded from the one declaration.
#[derive(Debug, Serialize, DerivePartialModel)]
#[turso(entity = "thread::Entity")]
pub struct ThreadSummary {
    pub id: i32,
    #[turso(from_col = "Title")]
    pub text: String,
    #[turso(from_expr = "Expr::col((\"thread\", \"title\")).concat(\" (open)\")")]
    pub open_text: String,
}
// --8<-- [end:partial_model]

/// A thread with its records in order.
#[derive(Debug, Serialize)]
pub struct ThreadWithRecords {
    pub thread: thread::Model,
    pub records: Vec<thread_record::Model>,
}

/// A thread with the tools it invoked.
#[derive(Debug, Serialize)]
pub struct ThreadWithTools {
    pub thread: thread::Model,
    pub tools: Vec<tool::Model>,
}
