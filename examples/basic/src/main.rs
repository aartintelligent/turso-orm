//! Starts the API: reads the settings, sets up tracing, opens the database,
//! applies the migrations, starts the replica's synchronisation task in the
//! `sync` mode and serves the router until `Ctrl-C`.
//!
//! The database is configured by `DATABASE__*` environment variables over
//! the defaults in `settings.rs`; without any, it lives in memory. The
//! server always listens on `127.0.0.1:7407`.
//!
//! ```text
//! cargo run
//! DATABASE__MODE=file DATABASE__PATH=agents.db cargo run
//! ```

use std::sync::Arc;

use basic::migration::{Migrator, MigratorTrait};
use basic::settings::{Mode, Settings};
use basic::{AppState, db, service, telemetry};
use tokio::net::TcpListener;

/// Where the server listens. Not a setting: change it here.
const LISTEN_ADDR: &str = "127.0.0.1:7407";

// --8<-- [start:main]
/// Reads the settings, migrates the database, starts the task and listens.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::load()?;
    telemetry::init();
    tracing::info!(mode = ?settings.mode, "opening the database");

    let db = db::connect(&settings).await?;
    Migrator::up(&db, None).await?;

    // The task lives as long as its handle: the state holds one reference
    // for the routes, `main` keeps another to shut it down after the server.
    let sync = match settings.mode {
        Mode::Sync => Some(Arc::new(service::sync::start(&db, &settings)?)),
        _ => None,
    };
    let mut state = AppState::new(db, settings);
    if let Some(sync) = &sync {
        state = state.with_sync(Arc::clone(sync));
    }

    let listener = TcpListener::bind(LISTEN_ADDR).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    axum::serve(listener, basic::app(state))
        .with_graceful_shutdown(async {
            // A failed signal handler would leave the server running
            // forever; stopping at once is the lesser evil.
            if let Err(err) = tokio::signal::ctrl_c().await {
                tracing::warn!(%err, "cannot listen for Ctrl-C; stopping");
            }
        })
        .await?;

    // The server has dropped its state, so the handle is ours alone: one
    // last push, then the task ends.
    if let Some(sync) = sync {
        match Arc::try_unwrap(sync) {
            Ok(sync) => sync.shutdown().await?,
            Err(_) => {
                tracing::warn!("the synchronisation task is still shared; skipping the final push")
            }
        }
    }
    Ok(())
}
// --8<-- [end:main]
