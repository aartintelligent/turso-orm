//! Keeping an embedded replica in step with Turso Cloud.
//!
//! A replica writes to its local file only: nothing leaves the machine until
//! `push`, and nothing arrives until `pull`. The driver exposes those two,
//! plus the checkpoint of the replica's log and its statistics, on the pool;
//! this module is the one place the example calls them.

use turso_orm::ReplicaStats;
use turso_orm::prelude::*;

// --8<-- [start:push_pull]
/// Sends the replica's local writes to Turso Cloud, then brings back what
/// other writers committed there; `true` when the pull changed the replica.
///
/// Both calls fail with `DbErr::Driver` on a database that is not a replica,
/// which is why the handler checks the mode before calling this.
pub async fn push_pull(db: &Database) -> Result<bool, DbErr> {
    db.push().await?;
    Ok(db.pull().await?)
}
// --8<-- [end:push_pull]

// --8<-- [start:checkpoint_stats]
/// Folds the replica's write-ahead log into its file. A replica that runs
/// for long calls this after each successful push, so that its files stay
/// bounded.
pub async fn checkpoint(db: &Database) -> Result<(), DbErr> {
    Ok(db.checkpoint().await?)
}

/// What the replica knows about itself: changes waiting to be pushed, the
/// size of its logs, when it last exchanged with the cloud.
pub async fn stats(db: &Database) -> Result<ReplicaStats, DbErr> {
    Ok(db.stats().await?)
}
// --8<-- [end:checkpoint_stats]
