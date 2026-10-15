//! The state every handler can reach: the pool and the settings.

use std::sync::Arc;

use axum::extract::FromRef;
use turso_orm::Database;

use crate::settings::Settings;

/// The shared state of the application.
///
/// `Database` is a pool, cheap to clone and safe to share; the settings are
/// behind an `Arc` so that cloning the state stays cheap too.
#[derive(Clone, Debug)]
pub struct AppState {
    /// The pool.
    pub db: Database,
    /// The settings the process started with.
    pub settings: Arc<Settings>,
}

impl AppState {
    /// Bundles a pool and the settings.
    pub fn new(db: Database, settings: Settings) -> Self {
        Self {
            db,
            settings: Arc::new(settings),
        }
    }
}

/// Lets a handler extract `State<Database>` without naming the whole state.
impl FromRef<AppState> for Database {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}
