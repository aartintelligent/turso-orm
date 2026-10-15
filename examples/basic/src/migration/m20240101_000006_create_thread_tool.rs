//! Creates the `thread_tool` table.

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
                    .table("thread_tool")
                    .if_not_exists()
                    .col(ColumnDef::integer("thread_id").not_null())
                    .col(ColumnDef::integer("tool_id").not_null())
                    .primary_key(["thread_id", "tool_id"])
                    .foreign_key(
                        ForeignKey::new(["thread_id"], "thread", ["id"])
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::new(["tool_id"], "tool", ["id"])
                            .on_delete(ForeignKeyAction::Cascade),
                    ),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table("thread_tool")).await
    }
}
