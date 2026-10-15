//! Starts the API: reads the settings, sets up tracing, opens the database,
//! applies the migrations and serves the router.
//!
//! The database is configured by `DATABASE__*` environment variables over
//! the defaults in `settings.rs`; without any, it lives in memory. The
//! server always listens on `127.0.0.1:7407`.
//!
//! ```text
//! cargo run
//! DATABASE__MODE=file DATABASE__PATH=agents.db cargo run
//! ```

use basic::migration::{Migrator, MigratorTrait};
use basic::settings::Settings;
use basic::{AppState, db, telemetry};
use tokio::net::TcpListener;

/// Where the server listens. Not a setting: change it here.
const LISTEN_ADDR: &str = "127.0.0.1:7407";

/// Reads the settings, migrates the database and listens.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::load()?;
    telemetry::init();
    tracing::info!(mode = ?settings.mode, "opening the database");

    let db = db::connect(&settings).await?;
    Migrator::up(&db, None).await?;

    let listener = TcpListener::bind(LISTEN_ADDR).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    axum::serve(listener, basic::app(AppState::new(db, settings))).await?;
    Ok(())
}
