//! Basic example: the entity API end to end against an in-memory database.
//!
//! `main` opens the database, creates the schema from the entities, then
//! hands the connection to the mutation walkthrough (which seeds the data)
//! and to the query walkthrough. Each step prints what it reads back.
//!
//! ```text
//! cargo run
//! ```

#![allow(clippy::print_stdout, reason = "examples are meant to print")]

mod entity;
mod mutation;
mod query;

use entity::{Bakery, Cake, Fruit};
use turso_orm::prelude::*;

/// Opens the database, builds the schema and runs both walkthroughs.
#[tokio::main]
async fn main() -> Result<(), DbErr> {
    let db = Database::connect(ConnectOptions::in_memory().foreign_keys(true)).await?;
    create_schema(&db).await?;

    println!("===== mutations =====\n");
    mutation::all_about_mutation(&db).await?;

    println!("\n===== queries =====\n");
    query::all_about_query(&db).await?;

    Ok(())
}

// --8<-- [start:schema]
/// Creates the three tables and the index declared on `cake`, in dependency order.
///
/// Foreign keys are derived from the `belongs_to` relations and the DDL is
/// `STRICT`-compatible, so the generated statements are what a migration
/// would contain.
async fn create_schema(db: &Database) -> Result<(), DbErr> {
    let schema = Schema::new();
    for stmt in [
        schema.create_table_from_entity(Bakery).to_statement(),
        schema.create_table_from_entity(Cake).to_statement(),
        schema.create_table_from_entity(Fruit).to_statement(),
    ] {
        println!("{}", stmt.sql);
        db.execute(stmt).await?;
    }
    for idx in schema.create_index_from_entity(Cake) {
        let stmt = idx.to_statement();
        println!("{}", stmt.sql);
        db.execute(stmt).await?;
    }
    println!();
    Ok(())
}
// --8<-- [end:schema]
