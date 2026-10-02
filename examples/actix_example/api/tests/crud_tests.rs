//! CRUD round-trip through the service layer against an in-memory database.
//!
//! The migrator builds the schema, so the test also covers the migrations.

use actix_example_api::service::{Mutation, Query};
use entity::post;
use migration::{Migrator, MigratorTrait};
use turso_orm::prelude::*;

/// Creates, reads, updates and deletes posts through the service functions.
#[tokio::test]
async fn crud_round_trip() {
    let db = Database::connect(ConnectOptions::in_memory())
        .await
        .unwrap();
    Migrator::up(&db, None).await.unwrap();
    // Start from an empty table so that the ids below are predictable.
    Mutation::delete_all_posts(&db).await.unwrap();

    let a = Mutation::create_post(&db, body("Title A", "Text A"))
        .await
        .unwrap();
    let b = Mutation::create_post(&db, body("Title B", "Text B"))
        .await
        .unwrap();
    assert_eq!(a.title, "Title A");
    assert_ne!(a.id, b.id);

    let found = Query::find_post_by_id(&db, a.id).await.unwrap().unwrap();
    assert_eq!(found, a);

    let (page, num_pages) = Query::find_posts_in_page(&db, 1, 1).await.unwrap();
    assert_eq!(page, vec![a.clone()]);
    assert_eq!(num_pages, 2);

    let updated = Mutation::update_post_by_id(&db, a.id, body("New A", "New text"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.title, "New A");
    assert_eq!(updated.text, "New text");
    assert!(
        Mutation::update_post_by_id(&db, 9999, body("x", "y"))
            .await
            .unwrap()
            .is_none()
    );

    assert_eq!(
        Mutation::delete_post(&db, b.id)
            .await
            .unwrap()
            .rows_affected,
        1
    );
    assert!(Query::find_post_by_id(&db, b.id).await.unwrap().is_none());
    assert_eq!(
        Mutation::delete_all_posts(&db).await.unwrap().rows_affected,
        1
    );
}

/// Builds a request body; the id is ignored by the service.
fn body(title: &str, text: &str) -> post::Model {
    post::Model {
        id: 0,
        title: title.to_owned(),
        text: text.to_owned(),
    }
}
