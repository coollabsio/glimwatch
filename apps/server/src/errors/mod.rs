//! Sentry-compatible error tracking. Dashboard routes use the existing session
//! cookie. Ingest routes keep the Sentry SDK paths and do not require a login.

mod http;
mod ingest;
mod seed;
mod store;

use axum::Router;
use axum::routing::{get, post};

use crate::state::AppState;

pub use seed::seed_demo;

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/error-projects",
            get(http::list_projects).post(http::create_project),
        )
        .route(
            "/error-projects/{id}",
            get(http::get_project).delete(http::delete_project),
        )
        .route("/error-issues", get(http::list_issues))
        .route("/error-issues/bulk", post(http::bulk_update))
        .route(
            "/error-issues/{id}",
            get(http::get_issue).patch(http::update_issue),
        )
        .route("/error-events/{id}", get(http::get_event))
        .route(
            "/{project_id}/envelope",
            post(ingest::ingest_envelope).options(ingest::ingest_options),
        )
        .route(
            "/{project_id}/envelope/",
            post(ingest::ingest_envelope).options(ingest::ingest_options),
        )
        .route(
            "/{project_id}/store",
            post(ingest::ingest_store).options(ingest::ingest_options),
        )
        .route(
            "/{project_id}/store/",
            post(ingest::ingest_store).options(ingest::ingest_options),
        )
        .route(
            "/{project_id}/security/",
            post(ingest::ingest_security).options(ingest::ingest_options),
        )
}
