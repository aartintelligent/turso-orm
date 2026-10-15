//! Seeds one model per seeded provider.

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
        let db = manager.get_connection();
        let mut models = Vec::new();
        for (provider, name, input_price) in [
            ("OpenAI", "gpt-4.1-mini", 0.4),
            ("Anthropic", "claude-sonnet-4", 3.0),
        ] {
            let provider = LlmProvider::find()
                .filter(llm_provider::Column::Name.eq(provider))
                .one(db)
                .await?
                .ok_or_else(|| DbErr::Migration(format!("provider {provider} is not seeded")))?;
            models.push(llm_model::ActiveModel {
                name: Set(name.to_owned()),
                input_price: Set(input_price),
                supports_tools: Set(true),
                provider_id: Set(provider.id),
                ..Default::default()
            });
        }
        LlmModel::insert_many(models).exec(db).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        LlmModel::delete_many()
            .filter(llm_model::Column::Name.is_in(["gpt-4.1-mini", "claude-sonnet-4"]))
            .exec(manager.get_connection())
            .await?;
        Ok(())
    }
}
