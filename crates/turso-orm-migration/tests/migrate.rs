//! Applying and reverting migrations against an in-memory Turso database.
//!
//! Three migrations cover the interesting paths: one creates a table and an
//! index, one alters the table and seeds data through the same transaction,
//! and one fails on purpose after creating a table so that the rollback
//! guarantee can be observed. Two migrators combine them: the regular one
//! for the full `up`, `down`, `status`, `refresh`, `fresh` and `reset`
//! cycle, and a failing one for the rollback test.
//!
//! ```text
//! cargo test -p turso-orm-migration --test migrate
//! ```

use turso_orm_migration::prelude::*;

/// Creates the `user` table and an index on its `email` column.
mod m20240101_000001_create_user {
    use turso_orm_migration::prelude::*;

    /// The migration.
    #[derive(DeriveMigrationName)]
    pub(crate) struct Migration;

    #[async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
            manager
                .create_table(
                    Table::create()
                        .table("user")
                        .col(ColumnDef::integer("id").primary_key().auto_increment())
                        .col(ColumnDef::text("email").not_null().unique_key()),
                )
                .await?;
            manager
                .create_index(
                    CreateIndex::new()
                        .name("idx-user-email")
                        .table("user")
                        .col("email"),
                )
                .await
        }

        async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
            manager.drop_table(Table::drop().table("user")).await
        }
    }
}

/// Adds a `name` column to `user` and seeds one row through the migration's transaction.
mod m20240102_000001_add_name {
    use turso_orm_migration::prelude::*;

    /// The migration.
    #[derive(DeriveMigrationName)]
    pub(crate) struct Migration;

    #[async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
            manager
                .alter_table(
                    Table::alter()
                        .table("user")
                        .add_column(ColumnDef::text("name")),
                )
                .await?;
            // A data migration goes through the same transaction as the
            // schema change, so it is rolled back with it on failure.
            manager
                .get_connection()
                .execute_unprepared("INSERT INTO user (email, name) VALUES ('seed@x.io', 'Seed')")
                .await?;
            Ok(())
        }

        async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
            manager
                .alter_table(Table::alter().table("user").drop_column("name"))
                .await
        }
    }
}

/// Creates an `orphan` table and then fails, to exercise the rollback.
mod m20240103_000001_failing {
    use turso_orm_migration::prelude::*;

    /// The migration; it has no `down` because it never completes.
    #[derive(DeriveMigrationName)]
    pub(crate) struct Migration;

    #[async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
            manager
                .create_table(
                    Table::create()
                        .table("orphan")
                        .col(ColumnDef::integer("id").primary_key()),
                )
                .await?;
            Err(DbErr::Migration("boom".into()))
        }
    }
}

/// The migrator with the two migrations that succeed.
struct Migrator;

#[async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_user::Migration),
            Box::new(m20240102_000001_add_name::Migration),
        ]
    }

    fn migration_table_name() -> &'static str {
        "schema_history"
    }
}

/// The migrator whose second migration fails.
struct FailingMigrator;

#[async_trait]
impl MigratorTrait for FailingMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_user::Migration),
            Box::new(m20240103_000001_failing::Migration),
        ]
    }
}

/// The full cycle: `up` applies both and seeds data, `down` with a step reverts one, and `refresh`, `fresh` and `reset` end in the expected states.
#[tokio::test]
async fn up_down_status_refresh() {
    let db = Database::connect(ConnectOptions::in_memory())
        .await
        .expect("open");
    Migrator::up(&db, None).await.expect("up");
    // The bookkeeping rows live in the table the migrator names.
    let versions = db
        .query_all(Statement::from_string("SELECT version FROM schema_history"))
        .await
        .expect("bookkeeping");
    assert_eq!(versions.len(), 2);
    let status = Migrator::status(&db).await.expect("status");
    assert_eq!(
        status
            .iter()
            .map(|s| (s.name.as_str(), s.applied))
            .collect::<Vec<_>>(),
        [
            ("m20240101_000001_create_user", true),
            ("m20240102_000001_add_name", true)
        ]
    );
    // The seed row written by the second migration survived its commit.
    let row = db
        .query_one(Statement::from_string(
            "SELECT name FROM user WHERE email = 'seed@x.io'",
        ))
        .await
        .expect("query")
        .expect("seeded");
    assert_eq!(row.get::<String>("name").unwrap(), "Seed");

    // One step down reverts only the newest migration.
    Migrator::down(&db, Some(1)).await.expect("down one");
    let status = Migrator::status(&db).await.expect("status");
    assert!(!status[1].applied);

    Migrator::refresh(&db).await.expect("refresh");
    assert_eq!(
        Migrator::get_applied_migrations(&db)
            .await
            .expect("applied")
            .len(),
        2
    );

    // `fresh` drops the bookkeeping table too and still ends fully applied.
    Migrator::fresh(&db).await.expect("fresh");
    assert_eq!(
        Migrator::get_applied_migrations(&db)
            .await
            .expect("applied")
            .len(),
        2
    );

    Migrator::reset(&db).await.expect("reset");
    assert!(
        Migrator::get_applied_migrations(&db)
            .await
            .expect("applied")
            .is_empty()
    );
}

/// A failed migration leaves neither its tables nor its bookkeeping row, while the earlier migration stays applied.
#[tokio::test]
async fn failed_migration_is_rolled_back() {
    let db = Database::connect(ConnectOptions::in_memory())
        .await
        .expect("open");
    let err = FailingMigrator::up(&db, None).await.expect_err("fails");
    assert!(matches!(err, DbErr::Migration(_)));
    let applied = FailingMigrator::get_applied_migrations(&db)
        .await
        .expect("applied");
    assert_eq!(applied, ["m20240101_000001_create_user"]);
    // The catalog shows the first migration's objects and nothing from the
    // failed one.
    let txn = db.begin().await.expect("begin");
    let manager = SchemaManager::new(&txn);
    assert!(manager.has_table("user").await.expect("has"));
    assert!(!manager.has_table("orphan").await.expect("has"));
    assert!(manager.has_index("idx-user-email").await.expect("has"));
    assert!(manager.has_column("user", "email").await.expect("has"));
    txn.rollback().await.expect("rollback");
}
