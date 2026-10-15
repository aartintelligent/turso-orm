//! The responses of the `/sync` routes.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use turso_orm::{ReplicaStats, SyncStatus};

/// The response of `POST /sync`: what the forced round did.
#[derive(Debug, Serialize)]
pub struct SyncReport {
    /// Whether the pull changed the replica.
    pub pulled: bool,
    /// Whether the round checkpointed.
    pub checkpointed: bool,
    /// Rounds run so far, this one included.
    pub rounds: u64,
}

impl From<SyncStatus> for SyncReport {
    fn from(status: SyncStatus) -> Self {
        Self {
            pulled: status.pulled,
            checkpointed: status.checkpointed,
            rounds: status.rounds,
        }
    }
}

/// The response of `GET /sync/stats`: the task's view of the replica.
#[derive(Debug, Serialize)]
pub struct ReplicaStatus {
    /// Rounds run so far, successful or not.
    pub rounds: u64,
    /// Rounds failed in a row; zero after a success.
    pub consecutive_failures: u32,
    /// The last round, in seconds since the Unix epoch.
    pub last_round_unix_time: Option<u64>,
    /// The last successful round, in seconds since the Unix epoch.
    pub last_success_unix_time: Option<u64>,
    /// The error of the last round, when it failed.
    pub last_error: Option<String>,
    /// The replica's stats at the end of the last successful round.
    pub stats: Option<ReplicaStatsReport>,
}

/// The replica's own statistics, as the last round read them.
#[derive(Debug, Serialize)]
pub struct ReplicaStatsReport {
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

/// Seconds since the Unix epoch, `None` for a time before it.
fn unix_time(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

impl From<SyncStatus> for ReplicaStatus {
    fn from(status: SyncStatus) -> Self {
        Self {
            rounds: status.rounds,
            consecutive_failures: status.consecutive_failures,
            last_round_unix_time: status.last_round.and_then(unix_time),
            last_success_unix_time: status.last_success.and_then(unix_time),
            last_error: status.last_error,
            stats: status.stats.map(ReplicaStatsReport::from),
        }
    }
}

impl From<ReplicaStats> for ReplicaStatsReport {
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
