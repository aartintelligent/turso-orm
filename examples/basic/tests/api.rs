//! Drives the router on an in-memory database with `oneshot` requests, without a port.
//!
//! Each test builds its own application over a fresh database migrated by
//! the crate's migrations, so the seeded catalog, two models, is there from
//! the start.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use basic::AppState;
use basic::entity::{Agent, LlmModel, Thread, ThreadRecord, ThreadTool, Tool};
use basic::migration::{Migrator, MigratorTrait};
use basic::settings::Settings;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use turso_orm::prelude::*;

/// A fresh in-memory database with the migrations applied, and the router over it.
async fn app() -> (Router, Database) {
    let db = Database::connect(ConnectOptions::in_memory().foreign_keys(true))
        .await
        .expect("open");
    Migrator::up(&db, None).await.expect("migrate");
    (
        basic::app(AppState::new(db.clone(), Settings::default())),
        db,
    )
}

/// Sends one request and decodes the JSON body, `Null` when there is none.
async fn call(app: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
        .expect("request");
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json")
    };
    (status, json)
}

/// Creates an agent and returns its id.
async fn agent(app: &Router, name: &str, model_id: Option<i32>) -> i32 {
    let (status, body) = call(
        app,
        "POST",
        "/agents",
        Some(json!({ "name": name, "model_id": model_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().expect("id") as i32
}

/// Opens a thread and returns its id.
async fn thread(app: &Router, agent_id: i32, title: &str, reviewer: Option<i32>) -> i32 {
    let (status, body) = call(
        app,
        "POST",
        "/threads",
        Some(json!({ "agent_id": agent_id, "title": title, "reviewer_agent_id": reviewer })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().expect("id") as i32
}

/// Models: inserts, updates, filters, stats and the bulk writes.
#[tokio::test]
async fn catalog_round_trip() {
    let (app, _db) = app().await;

    let (status, large) = call(
        &app,
        "POST",
        "/models",
        Some(json!({
            "name": "mistral-large", "input_price": 2.0, "supports_tools": true,
            "capabilities": { "context_window": 128000 }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(large["capabilities"]["context_window"], 128000);
    let (_, found) = call(&app, "GET", &format!("/models/{}", large["id"]), None).await;
    assert_eq!(found["name"], "mistral-large");

    let (status, priced) = call(
        &app,
        "PUT",
        &format!("/models/{}/price", large["id"]),
        Some(json!({ "input_price": 2.4 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(priced["input_price"], 2.4);
    let (status, _) = call(
        &app,
        "PUT",
        "/models/999/price",
        Some(json!({ "input_price": 1.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, by_name) = call(&app, "GET", "/models?name=mini", None).await;
    assert_eq!(by_name.as_array().expect("array").len(), 1);
    let (_, pricey) = call(&app, "GET", "/models?min_price=2.5", None).await;
    assert_eq!(pricey[0]["name"], "claude-sonnet-4");
    let (_, either) = call(&app, "GET", "/models?tools=true&min_price=2.5", None).await;
    assert_eq!(either.as_array().expect("array").len(), 3);
    let (_, stats) = call(&app, "GET", "/models/stats", None).await;
    assert_eq!(stats["count"], 3);
    assert_eq!(stats["most_expensive"], 3.0);

    let (status, batch) = call(
        &app,
        "POST",
        "/models/batch",
        Some(json!([
            { "name": "mistral-small", "input_price": 0.2, "supports_tools": false },
            { "name": "codestral", "input_price": 0.3, "supports_tools": true }
        ])),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(batch.as_array().expect("array").len(), 2);
    let (_, discounted) = call(
        &app,
        "PATCH",
        "/models/discount",
        Some(json!({ "input_price": 0.1 })),
    )
    .await;
    assert_eq!(discounted, 1);
    let (_, removed) = call(
        &app,
        "DELETE",
        "/models/below",
        Some(json!({ "input_price": 0.35 })),
    )
    .await;
    assert_eq!(removed.as_array().expect("array").len(), 2);
    let (_, stats) = call(&app, "GET", "/models/stats", None).await;
    assert_eq!(stats["count"], 3);
}

/// Agents: creation with the constraint errors mapped, paging, streaming, the model relation and `save`.
#[tokio::test]
async fn agents_pages_and_relations() {
    let (app, _db) = app().await;
    let planner = agent(&app, "planner", Some(1)).await;
    agent(&app, "writer", Some(1)).await;
    let reviewer = agent(&app, "reviewer", Some(2)).await;
    agent(&app, "intern", None).await;
    agent(&app, "archivist", None).await;

    let (status, _) = call(&app, "POST", "/agents", Some(json!({ "name": "planner" }))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = call(
        &app,
        "POST",
        "/agents",
        Some(json!({ "name": "lost", "model_id": 999 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, page) = call(&app, "GET", "/agents?size=2", None).await;
    assert_eq!(page["items"].as_array().expect("array").len(), 2);
    assert_eq!(page["items"][0]["model"]["name"], "gpt-4.1-mini");
    assert_eq!(page["pages"], 3);
    assert_eq!(page["total"], 5);
    let (_, last) = call(&app, "GET", "/agents?size=2&page=2", None).await;
    assert_eq!(last["items"][0]["agent"]["name"], "archivist");
    assert!(last["items"][0]["model"].is_null());
    let (_, names) = call(&app, "GET", "/agents/names", None).await;
    assert_eq!(
        names,
        json!(["archivist", "intern", "planner", "reviewer", "writer"])
    );

    let (_, with_model) = call(&app, "GET", &format!("/agents/{planner}"), None).await;
    assert_eq!(with_model["model"]["name"], "gpt-4.1-mini");
    let (status, _) = call(&app, "GET", "/agents/999", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, all) = call(&app, "GET", "/agents/with-models", None).await;
    assert_eq!(all.as_array().expect("array").len(), 5);
    assert!(all[3]["model"].is_null());
    let (_, pricey) = call(&app, "GET", "/agents/with-models?min_model_price=1.0", None).await;
    assert_eq!(pricey.as_array().expect("array").len(), 1);
    assert_eq!(pricey[0]["agent"]["id"], reviewer);
    let (_, per_model) = call(&app, "GET", "/models/with-agents", None).await;
    assert_eq!(per_model[0]["model"]["name"], "gpt-4.1-mini");
    assert_eq!(per_model[0]["agents"].as_array().expect("array").len(), 2);

    let (status, scout) = call(&app, "PUT", "/agents", Some(json!({ "name": "scout" }))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, renamed) = call(
        &app,
        "PUT",
        "/agents",
        Some(json!({ "id": scout["id"], "name": "scout-2" })),
    )
    .await;
    assert_eq!(renamed["id"], scout["id"]);
    assert_eq!(renamed["name"], "scout-2");
}

/// Threads: records in a transaction, forks, the reviewer, the tools, keyset pages and projections.
#[tokio::test]
async fn threads_records_forks_and_tools() {
    let (app, _db) = app().await;
    let planner = agent(&app, "planner", Some(1)).await;
    let writer = agent(&app, "writer", Some(1)).await;
    let root = thread(&app, planner, "Plan the launch", None).await;
    let (status, draft) = call(
        &app,
        "POST",
        "/threads/json",
        Some(json!({ "agent_id": writer, "title": "Draft the announcement", "status": "done", "ignored": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(draft["status"], "done");
    let reviewed = thread(&app, writer, "Review me", Some(planner)).await;

    let (status, records) = call(
        &app,
        "POST",
        &format!("/threads/{root}/records"),
        Some(json!([
            { "role": "user", "content": "Plan the launch." },
            { "role": "assistant", "tool_calls": [{ "name": "search" }] }
        ])),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{records}");
    assert_eq!(records[1]["seq"], 2);
    let (_, more) = call(
        &app,
        "POST",
        &format!("/threads/{root}/records"),
        Some(json!([{ "role": "tool", "content": "three results" }])),
    )
    .await;
    assert_eq!(more[0]["seq"], 3);
    let (_, with_records) = call(&app, "GET", &format!("/threads/{root}"), None).await;
    assert_eq!(with_records["records"].as_array().expect("array").len(), 3);
    assert_eq!(with_records["records"][2]["role"], "tool");
    let (status, _) = call(&app, "POST", "/threads/999/records", Some(json!([]))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, fork) = call(
        &app,
        "POST",
        &format!("/threads/{root}/fork"),
        Some(json!({ "agent_id": writer })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(fork["parent_id"], root);
    let (_, parent) = call(
        &app,
        "GET",
        &format!("/threads/{}/parent", fork["id"]),
        None,
    )
    .await;
    assert_eq!(parent["id"], root);
    let (_, forks) = call(&app, "GET", &format!("/threads/{root}/forks"), None).await;
    assert_eq!(forks.as_array().expect("array").len(), 1);
    let (_, by_planner) = call(&app, "GET", "/threads/reviewed-by/planner", None).await;
    assert_eq!(by_planner[0]["id"], reviewed);

    for name in ["search", "calculator"] {
        let (status, _) = call(
            &app,
            "POST",
            &format!("/threads/{root}/tools"),
            Some(json!({ "name": name })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    call(
        &app,
        "POST",
        &format!("/threads/{}/tools", fork["id"]),
        Some(json!({ "name": "search" })),
    )
    .await;
    let (_, tools) = call(&app, "GET", &format!("/threads/{root}/tools"), None).await;
    assert_eq!(tools.as_array().expect("array").len(), 2);
    let (_, searched) = call(&app, "GET", "/threads/using/search", None).await;
    assert_eq!(searched.as_array().expect("array").len(), 2);
    let (_, grouped) = call(&app, "GET", "/threads/with-tools", None).await;
    assert_eq!(grouped[0]["tools"].as_array().expect("array").len(), 2);
    assert_eq!(grouped[1]["tools"].as_array().expect("array").len(), 0);
    let (_, of_planner) = call(&app, "GET", &format!("/agents/{planner}/tools"), None).await;
    assert_eq!(of_planner.as_array().expect("array").len(), 2);
    let (_, of_everyone) = call(&app, "GET", "/agents/with-tools", None).await;
    assert_eq!(of_everyone[1]["tools"].as_array().expect("array").len(), 1);

    let (_, first) = call(&app, "GET", "/threads?size=2", None).await;
    assert_eq!(first.as_array().expect("array").len(), 2);
    let after = first[1]["id"].clone();
    let (_, rest) = call(&app, "GET", &format!("/threads?size=2&after={after}"), None).await;
    assert_eq!(rest.as_array().expect("array").len(), 2);
    let (_, summaries) = call(&app, "GET", "/threads/summaries", None).await;
    assert_eq!(summaries.as_array().expect("array").len(), 3);
    assert_eq!(summaries[0]["open_text"], "Plan the launch (open)");
    let (_, titles) = call(&app, "GET", "/threads/titles", None).await;
    assert_eq!(titles[0], json!([root, "Plan the launch"]));
    let (_, any_completed) = call(&app, "GET", "/threads/any-completed", None).await;
    assert_eq!(any_completed, true);
    let (_, purged) = call(&app, "DELETE", "/threads?prefix=Draft", None).await;
    assert_eq!(purged.as_array().expect("array").len(), 1);
}

/// An explicit transaction completes a thread with its closing record; savepoints let a
/// thread take existing and new tools in one unit of work.
#[tokio::test]
async fn transactions_and_savepoints() {
    let (app, db) = app().await;
    let planner = agent(&app, "planner", Some(1)).await;
    let root = thread(&app, planner, "Plan the launch", None).await;
    let (status, completed) = call(&app, "POST", &format!("/threads/{root}/complete"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(completed["status"], "done");
    let (_, with_records) = call(&app, "GET", &format!("/threads/{root}"), None).await;
    assert_eq!(with_records["records"][0]["role"], "system");
    let (status, _) = call(&app, "POST", "/threads/999/complete", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    call(
        &app,
        "POST",
        &format!("/threads/{root}/tools"),
        Some(json!({ "name": "search" })),
    )
    .await;
    let (status, opened) = call(
        &app,
        "POST",
        "/threads/with-tools",
        Some(json!({ "agent_id": planner, "title": "Draft", "tools": ["search", "calculator"] })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{opened}");
    assert_eq!(opened["tools"].as_array().expect("array").len(), 2);
    // `search` existed and was reused; `calculator` was created: two tools in all.
    assert_eq!(Tool::find().count(&db).await.expect("count"), 2);
    assert_eq!(ThreadTool::find().count(&db).await.expect("count"), 3);
}

// --8<-- [start:cascade]
/// Deleting an agent lets the engine cascade to its threads, their records and their junction rows.
#[tokio::test]
async fn deleting_an_agent_cascades() {
    let (app, db) = app().await;
    let planner = agent(&app, "planner", Some(1)).await;
    let root = thread(&app, planner, "Plan the launch", None).await;
    call(
        &app,
        "POST",
        &format!("/threads/{root}/records"),
        Some(json!([{ "role": "user", "content": "Go." }])),
    )
    .await;
    call(
        &app,
        "POST",
        &format!("/threads/{root}/tools"),
        Some(json!({ "name": "search" })),
    )
    .await;

    let (status, _) = call(&app, "DELETE", &format!("/agents/{planner}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(Thread::find().count(&db).await.expect("count"), 0);
    assert_eq!(ThreadRecord::find().count(&db).await.expect("count"), 0);
    assert_eq!(ThreadTool::find().count(&db).await.expect("count"), 0);
    // The tool itself is not owned by the thread and stays.
    assert_eq!(Tool::find().count(&db).await.expect("count"), 1);
    let (status, _) = call(&app, "DELETE", &format!("/agents/{planner}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
// --8<-- [end:cascade]

// --8<-- [start:schema]
/// Creates the six tables and their indexes from the entities, in dependency order.
///
/// Foreign keys are derived from the `belongs_to` relations and the DDL is
/// `STRICT`-compatible, so the generated statements are what a migration
/// would contain.
async fn create_schema(db: &Database) -> Result<(), DbErr> {
    let schema = Schema::new();
    for stmt in [
        schema.create_table_from_entity(LlmModel).to_statement(),
        schema.create_table_from_entity(Agent).to_statement(),
        schema.create_table_from_entity(Thread).to_statement(),
        schema.create_table_from_entity(ThreadRecord).to_statement(),
        schema.create_table_from_entity(Tool).to_statement(),
        schema.create_table_from_entity(ThreadTool).to_statement(),
    ] {
        db.execute(stmt).await?;
    }
    let indexes = schema
        .create_index_from_entity(Thread)
        .into_iter()
        .chain(schema.create_index_from_entity(ThreadRecord));
    for idx in indexes {
        db.execute(idx.to_statement()).await?;
    }
    Ok(())
}
// --8<-- [end:schema]

/// The schema generated from the entities serves the API as well as the migrated one.
#[tokio::test]
async fn schema_from_entities_serves_the_api() {
    let db = Database::connect(ConnectOptions::in_memory().foreign_keys(true))
        .await
        .expect("open");
    create_schema(&db).await.expect("schema");
    let app = basic::app(AppState::new(db, Settings::default()));
    let (_, stats) = call(&app, "GET", "/models/stats", None).await;
    assert_eq!(stats["count"], 0);
    let planner = agent(&app, "planner", None).await;
    let (status, _) = call(&app, "GET", &format!("/agents/{planner}"), None).await;
    assert_eq!(status, StatusCode::OK);
}

/// `POST /sync` is refused outside the `sync` mode: an in-memory database has no replica.
#[tokio::test]
async fn sync_needs_a_replica() {
    let (app, _db) = app().await;
    let (status, body) = call(&app, "POST", "/sync", None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "the database is not an embedded replica");
}
