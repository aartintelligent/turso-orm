//! A minimal migration CLI: `up`, `down`, `status`, `fresh`, `refresh`, `reset`.
//!
//! The database path comes from `DATABASE_URL` in the environment or in the
//! `.env` file next to the workspace root, like the server.
//!
//! ```text
//! cargo run -p migration -- status
//! ```

#![allow(clippy::print_stdout, reason = "a CLI reports on stdout")]

use migration::{Migrator, MigratorTrait};
use turso_orm_migration::prelude::*;

/// Parses the single command argument and runs it against the database.
#[tokio::main]
async fn main() -> Result<(), DbErr> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "actix_example.db".to_owned());
    let db = Database::connect(ConnectOptions::new(url)).await?;
    let command = std::env::args().nth(1).unwrap_or_else(|| "up".to_owned());
    match command.as_str() {
        "up" => Migrator::up(&db, None).await?,
        "down" => Migrator::down(&db, Some(1)).await?,
        "fresh" => Migrator::fresh(&db).await?,
        "refresh" => Migrator::refresh(&db).await?,
        "reset" => Migrator::reset(&db).await?,
        "status" => {}
        other => {
            eprintln!(
                "unknown command `{other}`; expected up, down, fresh, refresh, reset or status"
            );
            std::process::exit(2);
        }
    }
    for status in Migrator::status(&db).await? {
        let mark = if status.applied { "applied" } else { "pending" };
        println!("{mark:8} {}", status.name);
    }
    Ok(())
}
