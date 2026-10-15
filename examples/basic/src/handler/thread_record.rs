//! Routes of the records of a thread.

use crate::entity::thread_record;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use turso_orm::Database;

use crate::dto::thread_record::AppendRecord;
use crate::error::ApiError;
use crate::extract::{ApiJson, ApiPath};
use crate::handler::thread::load;
use crate::service::thread_record as service;

/// `POST /threads/{id}/records`.
pub async fn append(
    State(db): State<Database>,
    ApiPath(id): ApiPath<i32>,
    ApiJson(bodies): ApiJson<Vec<AppendRecord>>,
) -> Result<(StatusCode, Json<Vec<thread_record::Model>>), ApiError> {
    load(&db, id).await?;
    let records = service::append_records(&db, id, bodies).await?;
    Ok((StatusCode::CREATED, Json(records)))
}
