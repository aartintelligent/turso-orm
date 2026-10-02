//! The axum application: router, handlers and the error mapping.
//!
//! Handlers stay thin: they parse the request, call one service function and
//! turn the result into JSON. The `Database` is cheap to clone (it is a
//! handle on a pool) and travels in the router state.

pub mod service;

use std::env;

use axum::extract::{Path, Query as UrlQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use entity::post;
use migration::{Migrator, MigratorTrait};
use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

use service::{Mutation, Query};

/// Shared state of every handler.
#[derive(Clone)]
struct AppState {
    /// The connection pool.
    db: Database,
}

/// Query string of the list endpoint.
#[derive(Debug, Deserialize)]
struct ListParams {
    /// 1-based page number; defaults to the first page.
    page: Option<u64>,
    /// Page size; defaults to five.
    posts_per_page: Option<u64>,
}

/// Body of the list endpoint.
#[derive(Debug, Serialize)]
struct ListResponse {
    /// The posts of the requested page.
    posts: Vec<post::Model>,
    /// The page that was returned.
    page: u64,
    /// The page size that was used.
    posts_per_page: u64,
    /// The total number of pages.
    num_pages: u64,
}

/// What a handler returns when the database or the request is at fault.
#[derive(Debug)]
enum AppError {
    /// No row for the requested id.
    NotFound,
    /// Any database error.
    Db(DbErr),
}

impl From<DbErr> for AppError {
    fn from(err: DbErr) -> Self {
        Self::Db(err)
    }
}

// --8<-- [start:error]
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "post not found".to_owned()),
            // The driver classifies constraint errors, so a duplicate becomes
            // a client error instead of a 500.
            Self::Db(err) if err.is_unique_violation() => (StatusCode::CONFLICT, err.to_string()),
            Self::Db(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
// --8<-- [end:error]

// --8<-- [start:router]
/// Builds the router over an open database.
///
/// Exposed so that a test can drive the application without binding a port.
pub fn app(db: Database) -> Router {
    Router::new()
        .route("/posts", get(list_posts).post(create_post))
        .route(
            "/posts/{id}",
            get(get_post).put(update_post).delete(delete_post),
        )
        .with_state(AppState { db })
}
// --8<-- [end:router]

/// `GET /posts?page=1&posts_per_page=5`.
async fn list_posts(
    State(state): State<AppState>,
    UrlQuery(params): UrlQuery<ListParams>,
) -> Result<Json<ListResponse>, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let posts_per_page = params.posts_per_page.unwrap_or(5).clamp(1, 100);
    let (posts, num_pages) = Query::find_posts_in_page(&state.db, page, posts_per_page).await?;
    Ok(Json(ListResponse {
        posts,
        page,
        posts_per_page,
        num_pages,
    }))
}

// --8<-- [start:create]
/// `POST /posts` with `{"title": ..., "text": ...}`.
async fn create_post(
    State(state): State<AppState>,
    Json(body): Json<post::Model>,
) -> Result<(StatusCode, Json<post::Model>), AppError> {
    let created = Mutation::create_post(&state.db, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}
// --8<-- [end:create]

/// `GET /posts/{id}`.
async fn get_post(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<post::Model>, AppError> {
    Query::find_post_by_id(&state.db, id)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

/// `PUT /posts/{id}` with the same body as `POST`.
async fn update_post(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<post::Model>,
) -> Result<Json<post::Model>, AppError> {
    Mutation::update_post_by_id(&state.db, id, body)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

/// `DELETE /posts/{id}`.
async fn delete_post(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    let result = Mutation::delete_post(&state.db, id).await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

// --8<-- [start:start]
/// Reads `.env`, opens the database, applies pending migrations and serves.
#[tokio::main]
async fn start() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    dotenvy::dotenv().ok();
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "axum_example.db".to_owned());
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = env::var("PORT").unwrap_or_else(|_| "8000".to_owned());

    let db = Database::connect(ConnectOptions::new(db_url)).await?;
    Migrator::up(&db, None).await?;

    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}")).await?;
    tracing::info!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app(db)).await?;
    Ok(())
}
// --8<-- [end:start]

/// Runs the server and reports a startup failure on stderr.
pub fn main() {
    if let Err(err) = start() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
