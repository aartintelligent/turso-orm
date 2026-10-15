//! Creates `thread`, `thread_record`, `tool` and the `thread_tool` junction, with their indexes.

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
            .await?;
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
            .await?;
        manager
            .create_table(
                Table::create()
                    .table("tool")
                    .if_not_exists()
                    .col(ColumnDef::integer("id").primary_key().auto_increment())
                    .col(ColumnDef::text("name").not_null().unique_key())
                    .col(ColumnDef::text("description").null()),
            )
            .await?;
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
        for table in ["thread_tool", "tool", "thread_record", "thread"] {
            manager.drop_table(Table::drop().table(table)).await?;
        }
        Ok(())
    }
}
