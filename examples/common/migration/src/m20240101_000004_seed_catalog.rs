//! Seeds two providers and their models so that the examples have a catalog on first start.

use entity::prelude::{LlmModel, LlmProvider};
use entity::{llm_model, llm_provider};
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
        let db = manager.get_connection();
        let openai = llm_provider::ActiveModel {
            name: Set("OpenAI".to_owned()),
            base_url: Set("https://api.openai.com/v1".to_owned()),
            ..Default::default()
        }
        .insert(db)
        .await?;
        let anthropic = llm_provider::ActiveModel {
            name: Set("Anthropic".to_owned()),
            base_url: Set("https://api.anthropic.com".to_owned()),
            ..Default::default()
        }
        .insert(db)
        .await?;
        LlmModel::insert_many([
            llm_model::ActiveModel {
                name: Set("gpt-4.1-mini".to_owned()),
                input_price: Set(0.4),
                supports_tools: Set(true),
                provider_id: Set(openai.id),
                ..Default::default()
            },
            llm_model::ActiveModel {
                name: Set("claude-sonnet-4".to_owned()),
                input_price: Set(3.0),
                supports_tools: Set(true),
                provider_id: Set(anthropic.id),
                ..Default::default()
            },
        ])
        .exec(db)
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        // Deleting the providers cascades to their models.
        LlmProvider::delete_many()
            .filter(llm_provider::Column::Name.is_in(["OpenAI", "Anthropic"]))
            .exec(manager.get_connection())
            .await?;
        Ok(())
    }
}
