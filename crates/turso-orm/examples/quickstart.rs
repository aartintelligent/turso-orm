//! Minimal example: define an entity, create its table, insert a row and query it back.
//!
//! The entity module is the smallest complete one: a model with a primary
//! key and a unique column, an empty relation enum, and the default
//! `ActiveModelBehavior`. Everything runs against an in-memory database, so
//! the example needs no setup.
//!
//! ```text
//! cargo run -p turso-orm --example quickstart
//! ```

#![allow(clippy::print_stdout, reason = "examples are meant to print")]

// --8<-- [start:entity]
/// The `agent` entity.
mod agent {
    use turso_orm::prelude::*;

    /// A row of the `agent` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "agent")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub id: i32,
        #[turso(unique)]
        pub name: String,
        pub system_prompt: Option<String>,
    }

    /// The relations of `agent`; there are none.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
// --8<-- [end:entity]

use turso_orm::prelude::*;

/// Creates the table, inserts one agent and finds it again by a `LIKE` filter.
#[tokio::main]
async fn main() -> Result<(), DbErr> {
    let db = Database::connect(ConnectOptions::in_memory()).await?;
    db.execute(
        Schema::new()
            .create_table_from_entity(agent::Entity)
            .to_statement(),
    )
    .await?;

    // --8<-- [start:query]
    // `id` stays `NotSet`, so the database assigns it and `insert` returns
    // the stored row with the generated key.
    let planner = agent::ActiveModel {
        name: Set("planner".into()),
        system_prompt: Set(Some("You plan, step by step.".into())),
        ..Default::default()
    }
    .insert(&db)
    .await?;
    println!("inserted {planner:?}");

    let found = agent::Entity::find()
        .filter(agent::Column::Name.contains("plan"))
        .one(&db)
        .await?;
    println!("found {found:?}");
    // --8<-- [end:query]
    Ok(())
}
