//! The actix-web application: handlers, app configuration and the error mapping.
//!
//! Handlers stay thin: they parse the request, call one service function and
//! turn the result into JSON. The `Database` is cheap to clone (it is a
//! handle on a pool) and is shared through `web::Data`.

pub mod service;

use std::env;

use actix_web::http::StatusCode;
use actix_web::{App, HttpResponse, HttpServer, ResponseError, delete, get, post, put, web};
use entity::post;
use migration::{Migrator, MigratorTrait};
use serde::{Deserialize, Serialize};
use turso_orm::prelude::*;

use service::{Mutation, Query};

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

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => f.write_str("post not found"),
            Self::Db(err) => write!(f, "{err}"),
        }
    }
}

// --8<-- [start:error]
impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            // The driver classifies constraint errors, so a duplicate becomes
            // a client error instead of a 500.
            Self::Db(err) if err.is_unique_violation() => StatusCode::CONFLICT,
            Self::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .json(serde_json::json!({ "error": self.to_string() }))
    }
}
// --8<-- [end:error]

/// `GET /posts?page=1&posts_per_page=5`.
#[get("/posts")]
async fn list_posts(
    db: web::Data<Database>,
    params: web::Query<ListParams>,
) -> Result<web::Json<ListResponse>, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let posts_per_page = params.posts_per_page.unwrap_or(5).clamp(1, 100);
    let (posts, num_pages) = Query::find_posts_in_page(&db, page, posts_per_page).await?;
    Ok(web::Json(ListResponse {
        posts,
        page,
        posts_per_page,
        num_pages,
    }))
}

// --8<-- [start:create]
/// `POST /posts` with `{"title": ..., "text": ...}`.
#[post("/posts")]
async fn create_post(
    db: web::Data<Database>,
    body: web::Json<post::Model>,
) -> Result<HttpResponse, AppError> {
    let created = Mutation::create_post(&db, body.into_inner()).await?;
    Ok(HttpResponse::Created().json(created))
}
// --8<-- [end:create]

/// `GET /posts/{id}`.
#[get("/posts/{id}")]
async fn get_post(
    db: web::Data<Database>,
    id: web::Path<i32>,
) -> Result<web::Json<post::Model>, AppError> {
    Query::find_post_by_id(&db, id.into_inner())
        .await?
        .map(web::Json)
        .ok_or(AppError::NotFound)
}

/// `PUT /posts/{id}` with the same body as `POST`.
#[put("/posts/{id}")]
async fn update_post(
    db: web::Data<Database>,
    id: web::Path<i32>,
    body: web::Json<post::Model>,
) -> Result<web::Json<post::Model>, AppError> {
    Mutation::update_post_by_id(&db, id.into_inner(), body.into_inner())
        .await?
        .map(web::Json)
        .ok_or(AppError::NotFound)
}

/// `DELETE /posts/{id}`.
#[delete("/posts/{id}")]
async fn delete_post(
    db: web::Data<Database>,
    id: web::Path<i32>,
) -> Result<HttpResponse, AppError> {
    let result = Mutation::delete_post(&db, id.into_inner()).await?;
    if result.rows_affected == 0 {
        return Err(AppError::NotFound);
    }
    Ok(HttpResponse::NoContent().finish())
}

// --8<-- [start:configure]
/// Registers every handler; shared by the server and by tests.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_posts)
        .service(create_post)
        .service(get_post)
        .service(update_post)
        .service(delete_post);
}
// --8<-- [end:configure]

// --8<-- [start:start]
/// Reads `.env`, opens the database, applies pending migrations and serves.
#[actix_web::main]
async fn start() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    dotenvy::dotenv().ok();
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "actix_example.db".to_owned());
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = env::var("PORT").unwrap_or_else(|_| "8000".to_owned());

    let db = Database::connect(ConnectOptions::new(db_url)).await?;
    Migrator::up(&db, None).await?;

    tracing::info!("listening on http://{host}:{port}");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(db.clone()))
            .configure(configure)
    })
    .bind((host, port.parse::<u16>()?))?
    .run()
    .await?;
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
