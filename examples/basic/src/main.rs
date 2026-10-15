//! Basic example: the entity API end to end against an in-memory database.
//!
//! `main` opens the database, creates the schema from the shared entities,
//! then hands the connection to the mutation walkthrough (which seeds the
//! catalog and the agents), to the thread seed, to the query walkthrough and
//! to the relations walkthrough. Each step prints what it reads back.
//!
//! ```text
//! cargo run
//! ```

#![allow(clippy::print_stdout, reason = "examples are meant to print")]

mod mutation;
mod query;
mod relations;

use entity::prelude::{Agent, LlmModel, LlmProvider, Thread, ThreadRecord, ThreadTool, Tool};
use turso_orm::prelude::*;

/// Opens the database, builds the schema and runs the walkthroughs.
#[tokio::main]
async fn main() -> Result<(), DbErr> {
    let db = Database::connect(ConnectOptions::in_memory().foreign_keys(true)).await?;
    create_schema(&db).await?;

    println!("===== mutations =====\n");
    mutation::all_about_mutation(&db).await?;

    relations::seed(&db).await?;

    println!("\n===== queries =====\n");
    query::all_about_query(&db).await?;

    println!("\n===== relations =====\n");
    relations::all_about_relations(&db).await?;

    Ok(())
}

// --8<-- [start:schema]
/// Creates the seven tables and their indexes, in dependency order.
///
/// Foreign keys are derived from the `belongs_to` relations and the DDL is
/// `STRICT`-compatible, so the generated statements are what a migration
/// would contain.
async fn create_schema(db: &Database) -> Result<(), DbErr> {
    let schema = Schema::new();
    for stmt in [
        schema.create_table_from_entity(LlmProvider).to_statement(),
        schema.create_table_from_entity(LlmModel).to_statement(),
        schema.create_table_from_entity(Agent).to_statement(),
        schema.create_table_from_entity(Thread).to_statement(),
        schema.create_table_from_entity(ThreadRecord).to_statement(),
        schema.create_table_from_entity(Tool).to_statement(),
        schema.create_table_from_entity(ThreadTool).to_statement(),
    ] {
        println!("{}", stmt.sql);
        db.execute(stmt).await?;
    }
    let indexes = schema
        .create_index_from_entity(LlmModel)
        .into_iter()
        .chain(schema.create_index_from_entity(Thread))
        .chain(schema.create_index_from_entity(ThreadRecord));
    for idx in indexes {
        let stmt = idx.to_statement();
        println!("{}", stmt.sql);
        db.execute(stmt).await?;
    }
    println!();
    Ok(())
}
// --8<-- [end:schema]
