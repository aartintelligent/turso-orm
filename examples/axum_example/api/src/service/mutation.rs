//! Service functions that write.

use entity::post;
use entity::prelude::Post;
use turso_orm::prelude::*;
use turso_orm::query::DeleteResult;

/// Namespace for the write operations.
pub struct Mutation;

impl Mutation {
    /// Inserts a post from its JSON body and returns the stored row.
    pub async fn create_post(db: &Database, data: post::Model) -> Result<post::Model, DbErr> {
        post::ActiveModel {
            title: Set(data.title),
            text: Set(data.text),
            ..Default::default()
        }
        .insert(db)
        .await
    }

    // --8<-- [start:update]
    /// Replaces the title and text of an existing post; `None` when the id is unknown.
    pub async fn update_post_by_id(
        db: &Database,
        id: i32,
        data: post::Model,
    ) -> Result<Option<post::Model>, DbErr> {
        let Some(existing) = Post::find_by_id(id).one(db).await? else {
            return Ok(None);
        };
        let mut am: post::ActiveModel = existing.into();
        am.title = Set(data.title);
        am.text = Set(data.text);
        am.update(db).await.map(Some)
    }
    // --8<-- [end:update]

    /// Deletes one post by key; `rows_affected` is zero when the id is unknown.
    pub async fn delete_post(db: &Database, id: i32) -> Result<DeleteResult, DbErr> {
        Post::delete_by_id(id).exec(db).await
    }

    /// Deletes every post.
    pub async fn delete_all_posts(db: &Database) -> Result<DeleteResult, DbErr> {
        Post::delete_many().exec(db).await
    }
}
