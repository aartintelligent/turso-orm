//! Creates the `agent` table.

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
                    .table("agent")
                    .if_not_exists()
                    .col(ColumnDef::integer("id").primary_key().auto_increment())
                    .col(ColumnDef::text("name").not_null().unique_key())
                    .col(ColumnDef::text("system_prompt").null())
                    .col(ColumnDef::integer("model_id").null())
                    .foreign_key(
                        ForeignKey::new(["model_id"], "llm_model", ["id"])
                            .on_delete(ForeignKeyAction::SetNull),
                    ),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table("agent")).await
    }
}
