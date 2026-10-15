//! Starts the API: reads the settings, sets up tracing, opens the database,
//! applies the migrations and serves the router.
//!
//! Everything is configured by `BASIC__*` environment variables over the
//! defaults in `settings.rs`; without any, the database lives in memory and
//! the server listens on `127.0.0.1:3000`.
//!
//! ```text
//! cargo run
//! BASIC__DATABASE__MODE=file BASIC__DATABASE__PATH=agents.db cargo run
//! BASIC__DATABASE__MODE=remote BASIC__DATABASE__URL=libsql://... BASIC__DATABASE__AUTH_TOKEN=... cargo run
//! ```

use basic::settings::Settings;
use basic::{AppState, db, telemetry};
use migration::{Migrator, MigratorTrait};
use tokio::net::TcpListener;

/// Reads the settings, migrates the database and listens.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::load()?;
    telemetry::init(&settings.log);
    tracing::info!(mode = ?settings.database.mode, "opening the database");

    let db = db::connect(&settings.database).await?;
    Migrator::up(&db, None).await?;

    let listener = TcpListener::bind(&settings.server.addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    axum::serve(listener, basic::app(AppState::new(db, settings))).await?;
    Ok(())
}
