use axum::Json;
use axum::extract::{Path, Query, State};
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use statixo_errors::normalize_event;

use super::store::{self, EventSummary, IssueRow, Project};
use crate::error::AppError;
use crate::routes::require_user;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct NewProject {
    pub name: String,
    #[serde(default)]
    pub platform: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub platform: String,
    pub public_key: String,
    pub dsn: String,
    pub created_at: String,
}

pub async fn list_projects(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Vec<ProjectView>>, AppError> {
    require_user(&state, &jar).await?;
    let projects = store::list_projects(&state.db).await?;
    Ok(Json(
        projects
            .into_iter()
            .map(|project| project_view(&state, project))
            .collect(),
    ))
}

pub async fn create_project(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<NewProject>,
) -> Result<Json<ProjectView>, AppError> {
    require_user(&state, &jar).await?;
    let project = store::create_project(&state.db, &body.name, &body.platform).await?;
    Ok(Json(project_view(&state, project)))
}

pub async fn get_project(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<ProjectView>, AppError> {
    require_user(&state, &jar).await?;
    Ok(Json(project_view(
        &state,
        store::get_project(&state.db, &id).await?,
    )))
}

pub async fn delete_project(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    store::delete_project(&state.db, &id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn project_view(state: &AppState, project: Project) -> ProjectView {
    ProjectView {
        dsn: store::build_dsn(
            &state.config.public_origin,
            &project.public_key,
            &project.id,
        ),
        id: project.id,
        name: project.name,
        slug: project.slug,
        platform: project.platform,
        public_key: project.public_key,
        created_at: project.created_at,
    }
}

#[derive(Deserialize, Default)]
pub struct IssueFilter {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub environment: String,
    #[serde(default)]
    pub since: String,
    #[serde(default)]
    pub sort: String,
}

#[derive(Deserialize, Default)]
pub struct DetailFilter {
    #[serde(default)]
    pub range: String,
    #[serde(default)]
    pub environment: String,
    #[serde(default)]
    pub q: String,
}

#[derive(Deserialize)]
pub struct StatusBody {
    pub status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFeed {
    pub issues: Vec<IssueRow>,
    pub environments: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueDetail {
    #[serde(flatten)]
    pub issue: IssueRow,
    pub events: Vec<EventSummary>,
    pub latest: Option<Value>,
    pub histogram: Vec<i64>,
    pub period_events: i64,
    pub users: i64,
    pub tags: Vec<store::TagCount>,
}

pub async fn list_issues(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(filter): Query<IssueFilter>,
) -> Result<Json<IssueFeed>, AppError> {
    require_user(&state, &jar).await?;
    let status = match filter.status.as_str() {
        "" | "all" => String::new(),
        "unresolved" | "resolved" | "ignored" => filter.status,
        _ => {
            return Err(AppError::BadRequest(
                "Status filter must be unresolved, resolved, ignored, or all".into(),
            ));
        }
    };
    let environments = store::list_environments(&state.db, &filter.project_id).await?;
    let issues = store::list_issues(
        &state.db,
        &store::IssueQuery {
            status,
            project_id: filter.project_id,
            search: filter.q,
            environment: filter.environment,
            since: filter.since,
            sort: filter.sort,
        },
    )
    .await?;
    Ok(Json(IssueFeed {
        issues,
        environments,
    }))
}

pub async fn get_issue(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(filter): Query<DetailFilter>,
) -> Result<Json<IssueDetail>, AppError> {
    require_user(&state, &jar).await?;
    let issue = store::get_issue(&state.db, &id).await?;
    let (events, latest_payload, analytics) = store::issue_analytics(
        &state.db,
        &issue.id,
        if filter.range.is_empty() {
            "14d"
        } else {
            &filter.range
        },
        &filter.environment,
        &filter.q,
    )
    .await?;
    let latest = latest_payload.map(|payload| normalized_payload(&payload));
    Ok(Json(IssueDetail {
        issue,
        events,
        latest,
        histogram: analytics.histogram,
        period_events: analytics.period_events,
        users: analytics.users,
        tags: analytics.tags,
    }))
}

#[derive(Deserialize)]
pub struct BulkBody {
    pub ids: Vec<String>,
    pub status: String,
}

pub async fn bulk_update(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<BulkBody>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    let updated = store::update_many_status(&state.db, &body.ids, &body.status).await?;
    Ok(Json(serde_json::json!({ "updated": updated })))
}

pub async fn update_issue(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Json(body): Json<StatusBody>,
) -> Result<Json<IssueRow>, AppError> {
    require_user(&state, &jar).await?;
    Ok(Json(
        store::update_issue_status(&state.db, &id, &body.status).await?,
    ))
}

pub async fn get_event(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    let event = store::get_event(&state.db, &id).await?;
    Ok(Json(normalized_payload(&event.payload)))
}

fn normalized_payload(payload: &str) -> Value {
    let raw = serde_json::from_str(payload).unwrap_or(Value::Null);
    serde_json::to_value(normalize_event(raw)).unwrap_or(Value::Null)
}
