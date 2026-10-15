//! Bodies of the records of a thread.

use crate::entity::thread_record::Role;
use serde::Deserialize;
use turso_orm::prelude::*;

/// One item of the body of `POST /threads/{id}/records`.
#[derive(Debug, Deserialize)]
pub struct AppendRecord {
    pub role: Role,
    pub content: Option<String>,
    pub tool_calls: Option<Json>,
}
