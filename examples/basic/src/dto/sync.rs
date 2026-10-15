//! The response of `POST /sync`.

use serde::Serialize;

/// What a synchronisation did: the push always runs, `pulled` says whether
/// the pull brought changes down from the cloud.
#[derive(Debug, Serialize)]
pub struct SyncReport {
    /// Whether the pull changed the replica.
    pub pulled: bool,
}
