//! Seeds two models so that the API has a catalog on first start.

use turso_orm::prelude::*;
use turso_orm_migration::prelude::*;

use crate::entity::{LlmModel, llm_model};

/// The migration.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        // The connection is the migration's own transaction, so the seed is
        // rolled back together with the schema change if anything fails.
        LlmModel::insert_many([
            llm_model::ActiveModel {
                name: Set("gpt-4.1-mini".to_owned()),
                input_price: Set(0.4),
                supports_tools: Set(true),
                ..Default::default()
            },
            llm_model::ActiveModel {
                name: Set("claude-sonnet-4".to_owned()),
                input_price: Set(3.0),
                supports_tools: Set(true),
                ..Default::default()
            },
        ])
        .exec(manager.get_connection())
        .await?;
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
