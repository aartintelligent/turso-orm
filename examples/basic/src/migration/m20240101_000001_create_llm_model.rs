//! Creates the `llm_model` table.

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
                    .table("llm_model")
                    .if_not_exists()
                    .col(ColumnDef::integer("id").primary_key().auto_increment())
                    .col(ColumnDef::text("name").not_null())
                    .col(ColumnDef::real("input_price").not_null())
                    .col(ColumnDef::boolean("supports_tools").not_null())
                    .col(ColumnDef::json("capabilities").null()),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table("llm_model")).await
    }
}
