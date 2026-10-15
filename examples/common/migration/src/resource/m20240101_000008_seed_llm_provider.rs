//! Seeds two providers so that the examples have a catalog on first start.

use entity::llm_provider;
use entity::prelude::LlmProvider;
use turso_orm::prelude::*;
use turso_orm_migration::prelude::*;

/// The migration.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        // The connection is the migration's own transaction, so the seed is
        // rolled back together with the schema change if anything fails.
        LlmProvider::insert_many([
            llm_provider::ActiveModel {
                name: Set("OpenAI".to_owned()),
                base_url: Set("https://api.openai.com/v1".to_owned()),
                ..Default::default()
            },
            llm_provider::ActiveModel {
                name: Set("Anthropic".to_owned()),
                base_url: Set("https://api.anthropic.com".to_owned()),
                ..Default::default()
            },
        ])
        .exec(manager.get_connection())
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        LlmProvider::delete_many()
            .filter(llm_provider::Column::Name.is_in(["OpenAI", "Anthropic"]))
            .exec(manager.get_connection())
            .await?;
        Ok(())
    }
}
