//! Background synchronisation of an embedded replica, modeled by [`ReplicaSync`].
//!
//! The engine never pushes or pulls on its own, and [`Database`] offers the
//! four replica operations one call at a time. `ReplicaSync` runs them as a
//! loop: every `interval`, a round pushes the local writes, pulls the cloud's,
//! checkpoints when the policy says so and reads the replica's stats. The
//! outcome of the last round is kept where the application can read it
//! without touching the engine, and a round can be forced at any time.
//!
//! One round runs at a time, whether the loop or [`ReplicaSync::trigger`]
//! started it. A round that fails is recorded and logged at `warn`, and the
//! loop waits longer before the next one, doubling the interval up to
//! `max_backoff` until a round succeeds; the busy retries of the pool apply
//! inside each operation as they do everywhere else.
//!
//! The handle is the only hold on the task: dropping it stops the loop at
//! once, with no final push, and [`ReplicaSync::shutdown`] is the orderly
//! way out, which pushes one last time. Nothing starts without a call to
//! [`ReplicaSync::spawn`], and the four operations stay callable by hand
//! with or without the task.
//!
//! - [`ReplicaSync`]: the handle on the task;
//! - [`SyncPolicy`] and [`Checkpoint`]: how the rounds run;
//! - [`SyncStatus`]: what the last round did.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::{Mutex, Notify, watch};
use tokio::task::JoinHandle;

use crate::database::{Database, ReplicaStats};
use crate::error::{Error, Result};

/// When a round folds the replica's write-ahead log into its file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Checkpoint {
    /// Never: the application checkpoints by hand, or not at all.
    Never,
    /// After every push, which is when the log has nothing left to protect.
    #[default]
    AfterPush,
    /// When the write-ahead log is larger than this many bytes, read from
    /// the replica's stats at the end of the round.
    WalAbove(u64),
}

/// How a [`ReplicaSync`] runs its rounds.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SyncPolicy {
    /// The time between two rounds, from the end of one to the start of the
    /// next; thirty seconds by default.
    pub interval: Duration,
    /// When a round checkpoints; after every push by default.
    pub checkpoint: Checkpoint,
    /// Whether rounds only pull, for a replica that never writes; off by
    /// default.
    pub pull_only: bool,
    /// The longest wait between two rounds after failures: the interval
    /// doubles at each consecutive failure up to this; five minutes by
    /// default.
    pub max_backoff: Duration,
}

impl Default for SyncPolicy {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            checkpoint: Checkpoint::AfterPush,
            pull_only: false,
            max_backoff: Duration::from_mins(5),
        }
    }
}

impl SyncPolicy {
    /// The defaults: a round every thirty seconds, a checkpoint after every
    /// push, pushes on, a backoff capped at five minutes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the time between two rounds.
    #[must_use]
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Sets when a round checkpoints.
    #[must_use]
    pub fn checkpoint(mut self, checkpoint: Checkpoint) -> Self {
        self.checkpoint = checkpoint;
        self
    }

    /// Sets whether rounds only pull.
    #[must_use]
    pub fn pull_only(mut self, pull_only: bool) -> Self {
        self.pull_only = pull_only;
        self
    }

    /// Sets the longest wait between two rounds after failures.
    #[must_use]
    pub fn max_backoff(mut self, max_backoff: Duration) -> Self {
        self.max_backoff = max_backoff;
        self
    }

    /// The wait before the next round after `failures` consecutive failures.
    fn wait_after(&self, failures: u32) -> Duration {
        if failures == 0 {
            return self.interval;
        }
        // Sixteen doublings already exceed any sensible cap; the clamp keeps
        // the shift from overflowing.
        let factor = 2_u32.saturating_pow(failures.min(16));
        self.interval.saturating_mul(factor).min(self.max_backoff)
    }
}

/// What the last round of a [`ReplicaSync`] did, read with
/// [`ReplicaSync::status`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SyncStatus {
    /// Rounds completed so far, successful or not.
    pub rounds: u64,
    /// Rounds failed in a row; zero after a success.
    pub consecutive_failures: u32,
    /// When the last round ended.
    pub last_round: Option<SystemTime>,
    /// When the last successful round ended.
    pub last_success: Option<SystemTime>,
    /// Whether the pull of the last successful round changed the replica.
    pub pulled: bool,
    /// Whether the last successful round checkpointed.
    pub checkpointed: bool,
    /// The error of the last round, as text; `None` when it succeeded.
    pub last_error: Option<String>,
    /// The replica's stats, read at the end of the last successful round.
    pub stats: Option<ReplicaStats>,
}

/// What a successful round produced.
struct Round {
    /// Whether the pull changed the replica.
    pulled: bool,
    /// Whether the round checkpointed.
    checkpointed: bool,
    /// The stats read once the round was done.
    stats: ReplicaStats,
}

/// The state the handle and the task share.
struct Shared {
    /// The replica.
    db: Database,
    /// How rounds run.
    policy: SyncPolicy,
    /// Held for the duration of a round, so that the loop and `trigger`
    /// never run one at the same time.
    round: Mutex<()>,
    /// The outcome of the last round, for `status` and `subscribe`.
    status: watch::Sender<SyncStatus>,
    /// Told once by `shutdown`; the loop ends at its next wake-up.
    stop: Notify,
}

/// A background task that keeps an embedded replica in step with Turso
/// Cloud.
///
/// Created with [`spawn`](Self::spawn) on a [`Database`] opened with
/// [`ConnectOptions::sync`](crate::ConnectOptions::sync). The task holds a
/// clone of the pool, so the database lives at least as long as the task.
/// Dropping the handle stops the task at once; [`shutdown`](Self::shutdown)
/// stops it after one last push.
///
/// ```no_run
/// use std::time::Duration;
/// use turso_orm_driver::{ConnectOptions, Database, ReplicaSync, SyncPolicy};
///
/// # async fn run() -> Result<(), turso_orm_driver::Error> {
/// let db = Database::connect(
///     ConnectOptions::sync("replica.db", "turso://db.turso.io").auth_token("…"),
/// )
/// .await?;
/// let sync = ReplicaSync::spawn(
///     db.clone(),
///     SyncPolicy::new().interval(Duration::from_secs(10)),
/// )?;
/// // … serve requests through `db` …
/// sync.shutdown().await?;
/// # Ok(())
/// # }
/// ```
#[must_use = "dropping the handle stops the synchronisation task"]
pub struct ReplicaSync {
    /// The state shared with the task.
    shared: Arc<Shared>,
    /// The task, aborted on drop.
    task: JoinHandle<()>,
}

impl std::fmt::Debug for ReplicaSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplicaSync")
            .field("policy", &self.shared.policy)
            .field("status", &*self.shared.status.borrow())
            .finish_non_exhaustive()
    }
}

impl ReplicaSync {
    /// Starts the task on `db` with `policy`.
    ///
    /// The first round runs after one `interval`; call
    /// [`trigger`](Self::trigger) for one right away.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidOptions`] when `db` is not an embedded
    /// replica, and [`Error::Misuse`] when a `ReplicaSync` already runs on
    /// it: two loops would push and pull the same file against each other.
    pub fn spawn(db: Database, policy: SyncPolicy) -> Result<Self> {
        if !db.is_replica() {
            return Err(Error::InvalidOptions("not an embedded replica".into()));
        }
        if !db.claim_sync_task() {
            return Err(Error::Misuse(
                "a ReplicaSync already runs on this database".into(),
            ));
        }
        let (status, _) = watch::channel(SyncStatus::default());
        let shared = Arc::new(Shared {
            db,
            policy,
            round: Mutex::new(()),
            status,
            stop: Notify::new(),
        });
        let task = tokio::spawn(run(Arc::clone(&shared)));
        tracing::info!(
            interval = ?shared.policy.interval,
            checkpoint = ?shared.policy.checkpoint,
            pull_only = shared.policy.pull_only,
            "replica sync started"
        );
        Ok(Self { shared, task })
    }

    /// The policy the task runs with.
    pub fn policy(&self) -> &SyncPolicy {
        &self.shared.policy
    }

    /// What the last round did, without touching the engine.
    pub fn status(&self) -> SyncStatus {
        self.shared.status.borrow().clone()
    }

    /// A receiver that wakes up after every round with the new status.
    pub fn subscribe(&self) -> watch::Receiver<SyncStatus> {
        self.shared.status.subscribe()
    }

    /// Runs a round now and returns the status it left; a round already in
    /// progress is awaited first.
    ///
    /// # Errors
    ///
    /// Returns the error of the operation that failed, which the status
    /// records as well.
    pub async fn trigger(&self) -> Result<SyncStatus> {
        round(&self.shared).await?;
        Ok(self.status())
    }

    /// Stops the loop, waits for a round in progress, then pushes one last
    /// time unless the policy is pull-only.
    ///
    /// # Errors
    ///
    /// Returns the error of the final push; the loop is stopped either way.
    pub async fn shutdown(mut self) -> Result<()> {
        self.shared.stop.notify_one();
        // A finished or aborted task is not an error worth reporting here:
        // the loop has stopped either way.
        let _ = (&mut self.task).await;
        if !self.shared.policy.pull_only {
            self.shared.db.push().await?;
        }
        tracing::info!("replica sync shut down");
        Ok(())
    }
}

impl Drop for ReplicaSync {
    fn drop(&mut self) {
        self.task.abort();
        self.shared.db.release_sync_task();
    }
}

/// The loop: wait, run a round, repeat until told to stop.
async fn run(shared: Arc<Shared>) {
    loop {
        let failures = shared.status.borrow().consecutive_failures;
        let wait = shared.policy.wait_after(failures);
        // A stop that arrives during the wait ends the loop; the wait running
        // out starts a round.
        if tokio::time::timeout(wait, shared.stop.notified())
            .await
            .is_ok()
        {
            break;
        }
        if let Err(err) = round(&shared).await {
            tracing::warn!(%err, failures = failures + 1, "replica sync round failed");
        }
    }
    tracing::info!("replica sync stopped");
}

/// Runs one round under the lock and records its outcome in the status.
async fn round(shared: &Shared) -> Result<()> {
    let _one_at_a_time = shared.round.lock().await;
    let outcome = perform(shared).await;
    let now = SystemTime::now();
    shared.status.send_modify(|status| {
        status.rounds += 1;
        status.last_round = Some(now);
        match &outcome {
            Ok(round) => {
                status.consecutive_failures = 0;
                status.last_success = Some(now);
                status.pulled = round.pulled;
                status.checkpointed = round.checkpointed;
                status.last_error = None;
                status.stats = Some(round.stats.clone());
            }
            Err(err) => {
                status.consecutive_failures += 1;
                status.last_error = Some(err.to_string());
            }
        }
    });
    outcome.map(|_| ())
}

/// Pushes, pulls, checkpoints as the policy says and reads the stats.
async fn perform(shared: &Shared) -> Result<Round> {
    let db = &shared.db;
    let policy = &shared.policy;
    let pushed = if policy.pull_only {
        false
    } else {
        db.push().await?;
        true
    };
    let pulled = db.pull().await?;
    let mut stats = db.stats().await?;
    let checkpointed = match policy.checkpoint {
        Checkpoint::Never => false,
        Checkpoint::AfterPush => pushed,
        Checkpoint::WalAbove(limit) => stats.wal_bytes > limit,
    };
    if checkpointed {
        db.checkpoint().await?;
        stats = db.stats().await?;
    }
    tracing::debug!(
        pulled,
        checkpointed,
        pending = stats.pending_operations,
        "replica sync round done"
    );
    Ok(Round {
        pulled,
        checkpointed,
        stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::ConnectOptions;

    /// The wait grows by doubling after each failure and stops at the cap.
    #[test]
    fn wait_backs_off_up_to_the_cap() {
        let policy = SyncPolicy::new()
            .interval(Duration::from_secs(10))
            .max_backoff(Duration::from_mins(1));
        assert_eq!(policy.wait_after(0), Duration::from_secs(10));
        assert_eq!(policy.wait_after(1), Duration::from_secs(20));
        assert_eq!(policy.wait_after(2), Duration::from_secs(40));
        assert_eq!(policy.wait_after(3), Duration::from_mins(1));
        assert_eq!(policy.wait_after(40), Duration::from_mins(1));
    }

    /// A database that is not a replica is refused before any task starts.
    #[tokio::test]
    async fn spawn_needs_a_replica() {
        let db = Database::connect(ConnectOptions::in_memory())
            .await
            .expect("open");
        let err = ReplicaSync::spawn(db, SyncPolicy::new()).expect_err("not a replica");
        assert!(matches!(err, Error::InvalidOptions(_)), "{err:?}");
    }
}
