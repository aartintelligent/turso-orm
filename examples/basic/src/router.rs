//! Every route of the API, each bound to one handler.
//!
//! The state carries the pool and the settings; a handler extracts
//! `State<Database>` through `FromRef`, or `State<AppState>` for the rest.

use axum::Router;
use axum::routing::{get, patch, post, put};

use crate::handler::{agents, catalog, threads};
use crate::state::AppState;

/// Builds the router over `state`.
pub fn build(state: AppState) -> Router {
    Router::new()
        // The catalog.
        .route("/providers", post(catalog::create_provider))
        .route(
            "/providers/{id}/models",
            get(catalog::models_of_provider).delete(catalog::remove_provider_models),
        )
        .route(
            "/models",
            get(catalog::search_models).post(catalog::create_model),
        )
        .route("/models/batch", post(catalog::create_models))
        .route("/models/discount", patch(catalog::discount))
        .route("/models/stats", get(catalog::stats))
        .route("/models/with-agents", get(catalog::with_agents))
        .route("/models/{id}", get(catalog::find_model))
        .route("/models/{id}/price", put(catalog::update_price))
        // The agents.
        .route(
            "/agents",
            get(agents::list).post(agents::create).put(agents::save),
        )
        .route("/agents/names", get(agents::names))
        .route("/agents/with-models", get(agents::with_models))
        .route("/agents/with-tools", get(agents::with_tools))
        .route("/agents/{id}", get(agents::get).delete(agents::delete))
        .route("/agents/{id}/tools", get(agents::tools))
        // The threads.
        .route(
            "/threads",
            get(threads::list).post(threads::open).delete(threads::purge),
        )
        .route("/threads/json", post(threads::open_from_json))
        .route("/threads/with-tools", post(threads::open_with_tools))
        .route("/threads/summaries", get(threads::summaries))
        .route("/threads/titles", get(threads::titles))
        .route("/threads/any-completed", get(threads::any_completed))
        .route("/threads/with-tools", get(threads::with_tools))
        .route("/threads/reviewed-by/{name}", get(threads::reviewed_by))
        .route("/threads/using/{tool}", get(threads::using))
        .route("/threads/{id}", get(threads::get))
        .route("/threads/{id}/records", post(threads::append_records))
        .route("/threads/{id}/complete", post(threads::complete))
        .route("/threads/{id}/fork", post(threads::fork))
        .route("/threads/{id}/parent", get(threads::parent))
        .route("/threads/{id}/forks", get(threads::forks))
        .route("/threads/{id}/tools", get(threads::tools).post(threads::use_tool))
        .with_state(state)
}
