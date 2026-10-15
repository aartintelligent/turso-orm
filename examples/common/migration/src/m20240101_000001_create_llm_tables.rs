//! Creates the `llm_provider` and `llm_model` tables, and the index on the model's provider.

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
            .await?;
        manager
            .create_table(
                Table::create()
                    .table("llm_model")
                    .if_not_exists()
                    .col(ColumnDef::integer("id").primary_key().auto_increment())
                    .col(ColumnDef::text("name").not_null())
                    .col(ColumnDef::real("input_price").not_null())
                    .col(ColumnDef::boolean("supports_tools").not_null())
                    .col(ColumnDef::json("capabilities").null())
                    .col(ColumnDef::integer("provider_id").not_null())
                    .foreign_key(
                        ForeignKey::new(["provider_id"], "llm_provider", ["id"])
                            .on_delete(ForeignKeyAction::Cascade),
                    ),
            )
            .await?;
        manager
            .create_index(
                CreateIndex::new()
                    .name("idx-llm_model-provider_id")
                    .table("llm_model")
                    .col("provider_id"),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table("llm_model")).await?;
        manager
            .drop_table(Table::drop().table("llm_provider"))
            .await
    }
}
