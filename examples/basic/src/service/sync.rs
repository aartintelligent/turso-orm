//! Keeping an embedded replica in step with Turso Cloud.
//!
//! A replica writes to its local file only: nothing leaves the machine until
//! a push, and nothing arrives until a pull. The example leaves the rhythm
//! to the driver's `ReplicaSync` task, started here from the settings, and
//! keeps the checkpoint callable by hand.

use std::time::Duration;

use turso_orm::prelude::*;
use turso_orm::{Checkpoint, ReplicaSync, SyncPolicy, SyncStatus};

use crate::settings::Settings;

// --8<-- [start:start]
/// Starts the task that pushes, pulls and checkpoints the replica every
/// `sync_interval` seconds, for as long as the handle lives.
///
/// Fails with `DbErr::Driver` on a database that is not a replica, which is
/// why `main` calls this in the `sync` mode only.
pub fn start(db: &Database, settings: &Settings) -> Result<ReplicaSync, DbErr> {
    let policy = SyncPolicy::new()
        .interval(Duration::from_secs(settings.sync_interval))
        .checkpoint(Checkpoint::AfterPush);
    Ok(ReplicaSync::spawn(db.clone(), policy)?)
}
// --8<-- [end:start]

/// Runs a round now, serialised with the task's own rounds, and hands back
/// what it did.
pub async fn round(sync: &ReplicaSync) -> Result<SyncStatus, DbErr> {
    Ok(sync.trigger().await?)
}

// --8<-- [start:checkpoint]
/// Folds the replica's write-ahead log into its file, outside the task's
/// own rhythm.
pub async fn checkpoint(db: &Database) -> Result<(), DbErr> {
    Ok(db.checkpoint().await?)
}
// --8<-- [end:checkpoint]
