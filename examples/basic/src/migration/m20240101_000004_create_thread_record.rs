//! Creates the `thread_record` table.

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
                    .table("thread_record")
                    .if_not_exists()
                    .col(ColumnDef::integer("id").primary_key().auto_increment())
                    .col(ColumnDef::integer("thread_id").not_null())
                    .col(ColumnDef::integer("seq").not_null())
                    .col(ColumnDef::text("role").not_null())
                    .col(ColumnDef::text("content").null())
                    .col(ColumnDef::json("tool_calls").null())
                    .col(ColumnDef::integer("prompt_tokens").null())
                    .col(ColumnDef::integer("completion_tokens").null())
                    .foreign_key(
                        ForeignKey::new(["thread_id"], "thread", ["id"])
                            .on_delete(ForeignKeyAction::Cascade),
                    ),
            )
            .await?;
        manager
            .create_index(
                CreateIndex::new()
                    .name("idx-thread_record-thread_id")
                    .table("thread_record")
                    .col("thread_id"),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("thread_record"))
            .await
    }
}
