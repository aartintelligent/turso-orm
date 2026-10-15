//! A minimal migration CLI: `up`, `down`, `status`, `fresh`, `refresh`, `reset`.
//!
//! The database is the one the `DATABASE__*` settings describe, like the
//! server's; the `file` mode is the one that makes sense here.
//!
//! ```text
//! DATABASE__MODE=file cargo run --bin migrate -- status
//! ```

#![allow(clippy::print_stdout, reason = "a CLI reports on stdout")]

use basic::db;
use basic::migration::{Migrator, MigratorTrait};
use basic::settings::Settings;
use turso_orm::DbErr;

/// Parses the single command argument and runs it against the database.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = db::connect(&Settings::load()?).await?;
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
    for issue in Migrator::check(&db).await? {
        println!("warning  {issue}");
    }
    Ok::<(), DbErr>(()).map_err(Into::into)
}
