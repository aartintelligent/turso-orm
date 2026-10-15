//! Starts the API: opens the database, applies the migrations and serves the router.
//!
//! `DATABASE_URL` names a file; without it the database lives in memory for
//! as long as the process runs.
//!
//! ```text
//! cargo run
//! DATABASE_URL=agents.db cargo run
//! ```

#![allow(clippy::print_stdout, reason = "a server reports where it listens")]

use migration::{Migrator, MigratorTrait};
use tokio::net::TcpListener;
use turso_orm::prelude::*;

/// Opens the database, migrates it and listens on `127.0.0.1:3000`.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = match std::env::var("DATABASE_URL") {
        Ok(url) => ConnectOptions::new(url),
        Err(_) => ConnectOptions::in_memory(),
    };
    let db = Database::connect(options.foreign_keys(true)).await?;
    Migrator::up(&db, None).await?;

    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    println!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, basic::app(db)).await?;
    Ok(())
}
