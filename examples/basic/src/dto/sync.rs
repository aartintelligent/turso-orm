//! The responses of the `/sync` routes.

use serde::Serialize;
use turso_orm::ReplicaStats;

/// The response of `POST /sync`: the push always runs, `pulled` says whether
/// the pull brought changes down from the cloud.
#[derive(Debug, Serialize)]
pub struct SyncReport {
    /// Whether the pull changed the replica.
    pub pulled: bool,
}

/// The response of `GET /sync/stats`: the replica's view of its own state.
#[derive(Debug, Serialize)]
pub struct ReplicaStatus {
    /// Local changes waiting to be pushed.
    pub pending_operations: i64,
    /// The size in bytes of the write-ahead log, emptied by a checkpoint.
    pub wal_bytes: u64,
    /// The size in bytes of the revert log, which undoes what is not pushed yet.
    pub revert_wal_bytes: u64,
    /// The last pull, in seconds since the Unix epoch.
    pub last_pull_unix_time: Option<i64>,
    /// The last push, in seconds since the Unix epoch.
    pub last_push_unix_time: Option<i64>,
    /// The revision of the cloud database the replica is at.
    pub revision: Option<String>,
    /// Bytes sent to the cloud since the replica was opened.
    pub network_sent_bytes: u64,
    /// Bytes received from the cloud since the replica was opened.
    pub network_received_bytes: u64,
}

impl From<ReplicaStats> for ReplicaStatus {
    fn from(stats: ReplicaStats) -> Self {
        Self {
            pending_operations: stats.pending_operations,
            wal_bytes: stats.wal_bytes,
            revert_wal_bytes: stats.revert_wal_bytes,
            last_pull_unix_time: stats.last_pull_unix_time,
            last_push_unix_time: stats.last_push_unix_time,
            revision: stats.revision,
            network_sent_bytes: stats.network_sent_bytes,
            network_received_bytes: stats.network_received_bytes,
        }
    }
}
