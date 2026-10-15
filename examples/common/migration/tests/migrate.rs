//! The migrations apply, seed the catalog, and revert, on an in-memory database.

use entity::prelude::{Agent, LlmModel, LlmProvider};
use entity::{agent, llm_model};
use migration::{Migrator, MigratorTrait};
use turso_orm::prelude::*;

/// Applying every migration creates the schema and the seeded catalog; reverting them all
/// leaves nothing applied.
#[tokio::test]
async fn up_seeds_the_catalog_and_down_reverts() {
    let db = Database::connect(ConnectOptions::in_memory().foreign_keys(true))
        .await
        .expect("open");
    Migrator::up(&db, None).await.expect("up");
    let status = Migrator::status(&db).await.expect("status");
    assert_eq!(status.len(), 4);
    assert!(status.iter().all(|s| s.applied));

    assert_eq!(LlmProvider::find().count(&db).await.expect("count"), 2);
    let models = LlmModel::find()
        .order_by_asc(llm_model::Column::Id)
        .all(&db)
        .await
        .expect("models");
    assert_eq!(models.len(), 2);

    // The schema accepts the entities the examples write through.
    let planner = agent::ActiveModel {
        name: Set("planner".to_owned()),
        model_id: Set(Some(models[0].id)),
        ..Default::default()
    }
    .insert(&db)
    .await
    .expect("insert agent");
    assert_eq!(planner.model_id, Some(models[0].id));

    Migrator::down(&db, None).await.expect("down");
    let status = Migrator::status(&db).await.expect("status");
    assert!(status.iter().all(|s| !s.applied));
    assert!(Agent::find().count(&db).await.is_err());
}
