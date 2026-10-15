//! Creates the `thread` table.

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
                    .table("thread")
                    .if_not_exists()
                    .col(ColumnDef::integer("id").primary_key().auto_increment())
                    .col(ColumnDef::integer("agent_id").not_null())
                    .col(ColumnDef::integer("reviewer_agent_id").null())
                    .col(ColumnDef::integer("parent_id").null())
                    .col(ColumnDef::text("title").not_null())
                    .col(ColumnDef::text("status").not_null().default("open"))
                    .foreign_key(
                        ForeignKey::new(["agent_id"], "agent", ["id"])
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::new(["reviewer_agent_id"], "agent", ["id"])
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::new(["parent_id"], "thread", ["id"])
                            .on_delete(ForeignKeyAction::Cascade),
                    ),
            )
            .await?;
        manager
            .create_index(
                CreateIndex::new()
                    .name("idx-thread-agent_id")
                    .table("thread")
                    .col("agent_id"),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table("thread")).await
    }
}
