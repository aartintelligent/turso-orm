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
/// The `user` entity.
mod user {
    use turso_orm::prelude::*;

    /// A row of the `user` table.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[turso(table_name = "user")]
    pub(crate) struct Model {
        #[turso(primary_key)]
        pub id: i32,
        #[turso(unique)]
        pub email: String,
        pub name: Option<String>,
    }

    /// The relations of `user`; there are none.
    #[derive(Copy, Clone, Debug, DeriveRelation)]
    pub(crate) enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
// --8<-- [end:entity]

use turso_orm::prelude::*;

/// Creates the table, inserts one user and finds it again by a `LIKE` filter.
#[tokio::main]
async fn main() -> Result<(), DbErr> {
    let db = Database::connect(ConnectOptions::in_memory()).await?;
    db.execute(
        Schema::new()
            .create_table_from_entity(user::Entity)
            .to_statement(),
    )
    .await?;

    // --8<-- [start:query]
    // `id` stays `NotSet`, so the database assigns it and `insert` returns
    // the stored row with the generated key.
    let alice = user::ActiveModel {
        email: Set("alice@example.com".into()),
        name: Set(Some("Alice".into())),
        ..Default::default()
    }
    .insert(&db)
    .await?;
    println!("inserted {alice:?}");

    let found = user::Entity::find()
        .filter(user::Column::Email.contains("example"))
        .one(&db)
        .await?;
    println!("found {found:?}");
    // --8<-- [end:query]
    Ok(())
}
