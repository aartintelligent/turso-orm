//! Seeds two posts so that the API has something to list on first start.

use entity::post;
use entity::prelude::Post;
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
        Post::insert_many([
            post::ActiveModel {
                title: Set("Hello, Turso".to_owned()),
                text: Set("The first post, inserted by a migration.".to_owned()),
                ..Default::default()
            },
            post::ActiveModel {
                title: Set("Second post".to_owned()),
                text: Set("Also seeded.".to_owned()),
                ..Default::default()
            },
        ])
        .exec(manager.get_connection())
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        Post::delete_many()
            .filter(post::Column::Title.is_in(["Hello, Turso", "Second post"]))
            .exec(manager.get_connection())
            .await?;
        Ok(())
    }
}
