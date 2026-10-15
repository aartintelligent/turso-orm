//! Bodies of the tools a thread invokes.

use serde::Deserialize;

/// The body of `POST /threads/{id}/tools`.
#[derive(Debug, Deserialize)]
pub struct UseTool {
    pub name: String,
}
