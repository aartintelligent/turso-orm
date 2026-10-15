//! The state every handler can reach: the pool, the settings and, in the
//! `sync` mode, the replica's synchronisation task.

use std::sync::Arc;

use axum::extract::FromRef;
use turso_orm::{Database, DbErr, ReplicaSync};

use crate::settings::Settings;

/// The shared state of the application.
///
/// `Database` is a pool, cheap to clone and safe to share; the settings and
/// the task are behind an `Arc` so that cloning the state stays cheap too.
/// The fields are private: a handler reads them through the accessors, and
/// nothing outside this module decides what the state holds.
#[derive(Clone, Debug)]
pub struct AppState {
    /// The pool.
    db: Database,
    /// The database settings the process started with.
    settings: Arc<Settings>,
    /// The task that pushes and pulls the replica; `None` outside the `sync`
    /// mode.
    sync: Option<Arc<ReplicaSync>>,
}

impl AppState {
    /// Bundles a pool and the settings, with no synchronisation task.
    pub fn new(db: Database, settings: Settings) -> Self {
        Self {
            db,
            settings: Arc::new(settings),
            sync: None,
        }
    }

    /// Adds the synchronisation task the handlers drive.
    #[must_use]
    pub fn with_sync(mut self, sync: Arc<ReplicaSync>) -> Self {
        self.sync = Some(sync);
        self
    }

    /// The pool.
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// The database settings the process started with.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The synchronisation task, when the database is a replica.
    pub fn sync(&self) -> Option<&ReplicaSync> {
        self.sync.as_deref()
    }

    /// Checks that the database answers, for a health probe.
    pub async fn readiness(&self) -> Result<(), DbErr> {
        Ok(self.db.ping().await?)
    }
}

/// Lets a handler extract `State<Database>` without naming the whole state.
impl FromRef<AppState> for Database {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}
