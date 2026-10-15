//! Every route of the API, each bound to one handler; the paths are spelled
//! here and nowhere else.
//!
//! The state carries the pool, the settings and the task; a handler extracts
//! `State<Database>` through `FromRef`, or `State<AppState>` when it needs
//! more than the pool.

use axum::Router;
use axum::routing::{delete, get, patch, post, put};

use crate::handler::{agent, health, llm_model, sync, thread, thread_record, thread_tool};
use crate::state::AppState;

/// Builds the router over `state`.
pub fn build(state: AppState) -> Router {
    Router::new()
        // The process.
        .route("/health", get(health::check))
        .route("/sync", post(sync::run))
        .route("/sync/checkpoint", post(sync::checkpoint))
        .route("/sync/stats", get(sync::stats))
        // llm_model
        .route(
            "/models",
            get(llm_model::search).post(llm_model::create),
        )
        .route("/models/batch", post(llm_model::create_many))
        .route("/models/below", delete(llm_model::remove_below))
        .route("/models/discount", patch(llm_model::discount))
        .route("/models/stats", get(llm_model::stats))
        .route("/models/with-agents", get(llm_model::with_agents))
        .route("/models/{id}", get(llm_model::find))
        .route("/models/{id}/price", put(llm_model::update_price))
        // agent
        .route(
            "/agents",
            get(agent::list).post(agent::create).put(agent::save),
        )
        .route("/agents/names", get(agent::names))
        .route("/agents/with-models", get(agent::with_models))
        .route("/agents/with-tools", get(agent::with_tools))
        .route("/agents/{id}", get(agent::get).delete(agent::delete))
        .route("/agents/{id}/tools", get(agent::tools))
        // thread
        .route(
            "/threads",
            get(thread::list).post(thread::open).delete(thread::purge),
        )
        .route("/threads/json", post(thread::open_from_json))
        .route("/threads/with-tools", post(thread::open_with_tools))
        .route("/threads/summaries", get(thread::summaries))
        .route("/threads/titles", get(thread::titles))
        .route("/threads/any-completed", get(thread::any_completed))
        .route("/threads/reviewed-by/{name}", get(thread::reviewed_by))
        .route("/threads/{id}", get(thread::get))
        .route("/threads/{id}/complete", post(thread::complete))
        .route("/threads/{id}/fork", post(thread::fork))
        .route("/threads/{id}/parent", get(thread::parent))
        .route("/threads/{id}/forks", get(thread::forks))
        // thread_record
        .route("/threads/{id}/records", post(thread_record::append))
        // thread_tool
        .route("/threads/with-tools", get(thread_tool::with_tools))
        .route("/threads/using/{tool}", get(thread_tool::using))
        .route(
            "/threads/{id}/tools",
            get(thread_tool::tools).post(thread_tool::use_tool),
        )
        .with_state(state)
}
