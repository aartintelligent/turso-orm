//! Read-only service functions.

use entity::post;
use entity::prelude::Post;
use turso_orm::prelude::*;

/// Namespace for the read operations.
pub struct Query;

impl Query {
    /// Finds one post by key.
    pub async fn find_post_by_id(db: &Database, id: i32) -> Result<Option<post::Model>, DbErr> {
        Post::find_by_id(id).one(db).await
    }

    /// Returns the posts of a 1-based page and the total number of pages.
    pub async fn find_posts_in_page(
        db: &Database,
        page: u64,
        posts_per_page: u64,
    ) -> Result<(Vec<post::Model>, u64), DbErr> {
        let paginator = Post::find()
            .order_by_asc(post::Column::Id)
            .paginate(db, posts_per_page);
        let num_pages = paginator.num_pages().await?;
        let posts = paginator.fetch_page(page.saturating_sub(1)).await?;
        Ok((posts, num_pages))
    }
}
