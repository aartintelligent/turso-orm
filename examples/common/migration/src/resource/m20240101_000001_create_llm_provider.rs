//! Creates the `llm_provider` table.

use turso_orm_migration::prelude::*;

/// The migration.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("llm_provider")
                    .if_not_exists()
                    .col(ColumnDef::integer("id").primary_key().auto_increment())
                    .col(ColumnDef::text("name").not_null().unique_key())
                    .col(ColumnDef::text("base_url").not_null()),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("llm_provider"))
            .await
    }
}
