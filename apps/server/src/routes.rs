use std::collections::BTreeMap;

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post, put};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use owlee_domain::{
    Assertion, CheckType, HeaderPair, IncidentImpact, IncidentStatus, JobType, MonitorInput,
    MonitorState, NotificationEvent, NotificationPayload, OpenIncident, page_banner,
    resolved_at_after_update, validate_monitor, validate_slug,
};
use owlee_platform::{
    Id, PasswordError, TimestampMillis, generate_token, hash_token, normalize_email,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::trace::TraceLayer;

use crate::engine;
use crate::error::AppError;
use crate::instance_settings::{self, SmtpInput};
use crate::notify;
use crate::present::{self, Stats};
use crate::repo::{
    self, Agent, Channel, Incident, IncidentUpdate, Maintenance, Monitor, Page, User,
};
use crate::seed::{DEMO_EMAIL, DEMO_PASSWORD};
use crate::setup::{self, NewOwner};
use crate::state::{self, AppState};

const COOKIE: &str = "owlee_session";
/// Older session cookies. A later login replaces them with `owlee_session`.
const LEGACY_COOKIES: &[&str] = &["glimwatch_session", "status_session"];

fn session_cookie_names() -> impl Iterator<Item = &'static str> {
    std::iter::once(COOKIE).chain(LEGACY_COOKIES.iter().copied())
}
const DAY_MS: i64 = 86_400_000;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(ready))
        .route("/meta", get(meta))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/setup/status", get(setup_status))
        .route("/setup/complete", post(setup_complete))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/admin/settings", get(admin_settings))
        .route("/admin/settings/registration", put(set_registration))
        .route("/admin/settings/smtp", put(save_smtp).delete(delete_smtp))
        .route("/admin/users", get(admin_users))
        .route("/admin/users/{id}/admin", put(set_installation_admin))
        .route("/overview", get(overview))
        .route("/monitors", get(list_monitors).post(create_monitor))
        .route(
            "/monitors/{id}",
            get(get_monitor).put(update_monitor).delete(delete_monitor),
        )
        .route("/monitors/{id}/checks", post(run_check))
        .route("/monitors/{id}/active", post(set_active))
        .route("/agents", get(list_agents).post(create_agent))
        .route("/agents/{id}", axum::routing::delete(delete_agent))
        .route("/agent/checks", post(agent_checks))
        .route("/agent/heartbeat", post(agent_heartbeat))
        .route("/agent/results", post(agent_result))
        .route("/pages", get(list_pages).post(create_page))
        .route(
            "/pages/{id}",
            get(get_page).put(update_page).delete(delete_page),
        )
        .route("/incidents", get(list_incidents).post(create_incident))
        .route("/incidents/{id}", get(get_incident).delete(delete_incident))
        .route("/incidents/{id}/updates", post(add_update))
        .route(
            "/maintenances",
            get(list_maintenances).post(create_maintenance),
        )
        .route(
            "/maintenances/{id}",
            axum::routing::delete(delete_maintenance),
        )
        .route("/notifications", get(list_channels).post(create_channel))
        .route(
            "/notifications/{id}",
            axum::routing::put(update_channel).delete(delete_channel),
        )
        .route("/notifications/{id}/test", post(test_channel))
        .route("/public/{slug}", get(public_page))
        .merge(crate::insights::api_routes())
        .merge(crate::errors::api_routes())
        .fallback(api_missing);
    Router::new()
        .nest("/api", api)
        .merge(crate::insights::public_routes())
        .fallback(crate::insights::fallback)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn ready(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    sqlx::query("SELECT 1").execute(state.db.pool()).await?;
    Ok(Json(
        json!({ "status": "ok", "database": state.config.backend_name() }),
    ))
}

async fn meta(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let setup_complete = setup::is_complete(&state.db).await?;
    let settings = instance_settings::load(&state.db).await?;
    let demo = if state.config.environment.as_str() == "development" && setup_complete {
        json!({ "email": DEMO_EMAIL, "password": DEMO_PASSWORD })
    } else {
        Value::Null
    };
    Ok(Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "signup_open": settings.signup_open(state.config.smtp.is_some(), setup_complete),
        "setup_complete": setup_complete,
        "database": state.config.backend_name(),
        "public_origin": state.config.public_origin,
        "demo": demo,
    })))
}

async fn api_missing() -> AppError {
    AppError::NotFound
}

#[derive(Deserialize)]
struct AuthBody {
    email: String,
    password: String,
    display_name: Option<String>,
}

async fn register(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<AuthBody>,
) -> Result<(CookieJar, Json<present::UserResponse>), AppError> {
    let account = account_fields(&body.email, body.display_name)?;
    if let Err(error) = state.passwords.validate(&body.password) {
        return Err(password_field(error));
    }
    // The first account is created only with the setup token. Later accounts need
    // the root user to open registration after a mail server is saved.
    let settings = instance_settings::load(&state.db).await?;
    let setup_complete = setup::is_complete(&state.db).await?;
    if !settings.signup_open(state.config.smtp.is_some(), setup_complete) {
        return Err(AppError::RegistrationClosed);
    }
    let count = repo::count_users(&state.db).await?;
    if repo::user_by_email(&state.db, &account.normalized)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(
            "An account with that email already exists.".into(),
        ));
    }
    let hash = state::hash_password(&state.passwords, body.password)
        .await
        .map_err(password_field)?;
    let now = TimestampMillis::now();
    let id = Id::new();
    let role = if count == 0 { "owner" } else { "member" };
    repo::insert_user(
        &state.db,
        id,
        &account.email,
        &account.normalized,
        &account.display_name,
        &hash,
        role,
        now,
    )
    .await?;
    let user = repo::user_by_email(&state.db, &account.normalized)
        .await?
        .ok_or(AppError::Internal)?;
    let jar = issue_session(&state, jar, &user, now).await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(user.id),
        "auth.register",
        "user",
        Some(&user.id.to_string()),
        now,
    )
    .await;
    Ok((jar, Json(user_response(&state, &user).await?)))
}

async fn setup_status(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({
        "complete": setup::is_complete(&state.db).await?,
    })))
}

#[derive(Deserialize)]
struct SetupBody {
    token: String,
    email: String,
    password: String,
    display_name: Option<String>,
}

async fn setup_complete(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SetupBody>,
) -> Result<(StatusCode, CookieJar, Json<present::UserResponse>), AppError> {
    let now = TimestampMillis::now();
    // A finished install refuses setup even when the body still carries the old token.
    // A wrong or expired token, while setup is still open, is unauthorized.
    if setup::is_complete(&state.db).await? {
        return Err(AppError::SetupUnavailable);
    }
    if !setup::setup_token_valid(&state.db, &body.token, now).await? {
        return Err(AppError::InvalidSetupToken);
    }
    let account = account_fields(&body.email, body.display_name)?;
    if let Err(error) = state.passwords.validate(&body.password) {
        return Err(password_field(error));
    }
    let password_hash = state::hash_password(&state.passwords, body.password)
        .await
        .map_err(password_field)?;
    let completed = setup::complete_setup(
        &state.db,
        NewOwner {
            token: body.token,
            email: account.email,
            normalized_email: account.normalized,
            display_name: account.display_name,
            password_hash,
        },
        TimestampMillis::now(),
    )
    .await
    .map_err(|error| match error {
        setup::SetupError::AlreadyComplete => AppError::SetupUnavailable,
        setup::SetupError::InvalidToken => AppError::InvalidSetupToken,
        setup::SetupError::EmailTaken => {
            AppError::Conflict("An account with that email already exists.".into())
        }
        setup::SetupError::Database(error) => AppError::from(error),
    })?;
    let jar = session_jar(&state, jar, completed.session_token);
    Ok((
        StatusCode::CREATED,
        jar,
        Json(user_response(&state, &completed.user).await?),
    ))
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<AuthBody>,
) -> Result<(CookieJar, Json<present::UserResponse>), AppError> {
    let normalized = normalize_email(&body.email);
    let Some(user) = repo::user_by_email(&state.db, &normalized).await? else {
        return Err(AppError::InvalidLogin);
    };
    let valid = state::verify_password(&state.passwords, body.password, user.password_hash.clone())
        .await
        .unwrap_or(false);
    if !valid {
        return Err(AppError::InvalidLogin);
    }
    let now = TimestampMillis::now();
    let jar = issue_session(&state, jar, &user, now).await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(user.id),
        "auth.login",
        "user",
        Some(&user.id.to_string()),
        now,
    )
    .await;
    Ok((jar, Json(user_response(&state, &user).await?)))
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), AppError> {
    for name in session_cookie_names() {
        if let Some(token) = jar.get(name).map(|cookie| cookie.value().to_owned()) {
            repo::revoke_token(&state.db, &hash_token(&token), TimestampMillis::now()).await?;
        }
    }
    let mut jar = jar;
    for name in session_cookie_names() {
        let mut expired = Cookie::from(name);
        expired.set_path("/");
        jar = jar.remove(expired);
    }
    Ok((jar, StatusCode::NO_CONTENT))
}

async fn me(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<present::UserResponse>, AppError> {
    let user = require_user(&state, &jar).await?;
    Ok(Json(user_response(&state, &user).await?))
}

async fn admin_settings(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    require_root(&state, &jar).await?;
    Ok(Json(settings_body(&state).await?))
}

#[derive(Deserialize)]
struct RegistrationBody {
    open: bool,
}

async fn set_registration(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<RegistrationBody>,
) -> Result<Json<Value>, AppError> {
    let actor = require_root(&state, &jar).await?;
    let now = TimestampMillis::now();
    instance_settings::set_registration_open(
        &state.db,
        body.open,
        state.config.smtp.is_some(),
        now,
    )
    .await
    .map_err(|error| match error {
        instance_settings::InstanceError::EmailNotConfigured => AppError::EmailNotConfigured,
        instance_settings::InstanceError::Database(error) => AppError::from(error),
    })?;
    let _ = repo::insert_audit(
        &state.db,
        Some(actor.id),
        if body.open {
            "instance.registration_opened"
        } else {
            "instance.registration_closed"
        },
        "installation",
        None,
        now,
    )
    .await;
    Ok(Json(settings_body(&state).await?))
}

#[derive(Deserialize)]
struct SmtpBody {
    host: String,
    port: Option<i64>,
    username: Option<String>,
    password: Option<String>,
    from: String,
}

async fn save_smtp(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SmtpBody>,
) -> Result<Json<Value>, AppError> {
    let actor = require_root(&state, &jar).await?;
    let input = smtp_input(&body)?;
    let now = TimestampMillis::now();
    instance_settings::save_smtp(&state.db, input, now).await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(actor.id),
        "instance.smtp_updated",
        "installation",
        None,
        now,
    )
    .await;
    Ok(Json(settings_body(&state).await?))
}

async fn delete_smtp(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    let actor = require_root(&state, &jar).await?;
    let now = TimestampMillis::now();
    instance_settings::clear_smtp(&state.db, now).await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(actor.id),
        "instance.smtp_removed",
        "installation",
        None,
        now,
    )
    .await;
    Ok(Json(settings_body(&state).await?))
}

async fn admin_users(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    require_installation_admin(&state, &jar).await?;
    let root_id = instance_settings::root_user_id(&state.db).await?;
    let accounts = repo::list_accounts(&state.db).await?;
    let users = accounts
        .iter()
        .map(|account| account_body(account, root_id))
        .collect::<Vec<_>>();
    Ok(Json(json!(users)))
}

#[derive(Deserialize)]
struct AdminBody {
    admin: bool,
}

async fn set_installation_admin(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
    Json(body): Json<AdminBody>,
) -> Result<Json<Value>, AppError> {
    let actor = require_root(&state, &jar).await?;
    let root_id = instance_settings::root_user_id(&state.db).await?;
    if root_id == Some(id) {
        return Err(AppError::Conflict(
            "The root user stays an installation admin.".into(),
        ));
    }
    let now = TimestampMillis::now();
    let role = if body.admin { "admin" } else { "member" };
    let updated = repo::set_user_role(&state.db, id, role, now).await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    let _ = repo::insert_audit(
        &state.db,
        Some(actor.id),
        if body.admin {
            "instance.admin_granted"
        } else {
            "instance.admin_revoked"
        },
        "user",
        Some(&id.to_string()),
        now,
    )
    .await;
    let accounts = repo::list_accounts(&state.db).await?;
    let account = accounts
        .iter()
        .find(|account| account.id == id)
        .ok_or(AppError::NotFound)?;
    Ok(Json(account_body(account, root_id)))
}

async fn settings_body(state: &AppState) -> Result<Value, AppError> {
    let settings = instance_settings::load(&state.db).await?;
    let env_smtp = state.config.smtp.is_some();
    Ok(json!({
        "registration_open": settings.registration_open,
        "smtp_ready": settings.smtp_ready(env_smtp),
        "smtp": smtp_body(&settings, state.config.smtp.as_ref()),
        "root_user": settings.root_user.as_ref().map(|user| json!({
            "id": user.id,
            "email": user.email,
            "display_name": user.display_name,
        })),
        "initialized": settings.initialized,
        "version": env!("CARGO_PKG_VERSION"),
        "database": state.config.backend_name(),
    }))
}

fn smtp_body(
    settings: &instance_settings::InstanceView,
    env: Option<&owlee_platform::SmtpConfig>,
) -> Value {
    if let Some(smtp) = &settings.smtp {
        return json!({
            "host": smtp.host,
            "port": smtp.port,
            "username": smtp.username,
            "from": smtp.from,
            "has_password": smtp.password.is_some(),
            "source": "saved",
        });
    }
    env.map(|smtp| {
        json!({
            "host": smtp.host,
            "port": smtp.port,
            "username": smtp.username,
            "from": smtp.from,
            "has_password": smtp.password.is_some(),
            "source": "environment",
        })
    })
    .unwrap_or(Value::Null)
}

fn account_body(account: &repo::Account, root_id: Option<Id>) -> Value {
    let root = root_id == Some(account.id);
    json!({
        "id": account.id,
        "email": account.email,
        "display_name": account.display_name,
        "role": account.role,
        "root": root,
        "installation_admin": root || account.role == "owner" || account.role == "admin",
    })
}

fn smtp_input(body: &SmtpBody) -> Result<SmtpInput, AppError> {
    let host = body.host.trim();
    if host.is_empty()
        || host.len() > 253
        || host.contains(char::is_whitespace)
        || host.contains("://")
    {
        return Err(AppError::field("host", "Enter the mail server hostname."));
    }
    let port = match body.port {
        None => 587,
        Some(port) if (1..=65535).contains(&port) => u16::try_from(port).unwrap_or(587),
        Some(_) => return Err(AppError::field("port", "Enter a port from 1 to 65535.")),
    };
    let from = body.from.trim();
    if !valid_email(from) {
        return Err(AppError::field("from", "Enter a valid From address."));
    }
    let username = body
        .username
        .as_deref()
        .map(str::trim)
        .filter(|username| !username.is_empty())
        .map(str::to_owned);
    if username
        .as_ref()
        .is_some_and(|username| username.len() > 200)
    {
        return Err(AppError::field("username", "Use at most 200 characters."));
    }
    let password = body
        .password
        .as_deref()
        .map(str::trim)
        .filter(|password| !password.is_empty())
        .map(str::to_owned);
    if password
        .as_ref()
        .is_some_and(|password| password.len() > 500)
    {
        return Err(AppError::field("password", "Use at most 500 characters."));
    }
    Ok(SmtpInput {
        host: host.to_owned(),
        port,
        username,
        password,
        from: from.to_owned(),
    })
}

async fn overview(State(state): State<AppState>, jar: CookieJar) -> Result<Json<Value>, AppError> {
    let _user = require_user(&state, &jar).await?;
    let stats = load_stats(&state).await?;
    let monitors = repo::list_monitors(&state.db).await?;
    let monitors = monitors
        .iter()
        .map(|monitor| present::monitor_response(monitor, stats_view(&stats), 30, Vec::new()))
        .collect::<Vec<_>>();
    let incidents = open_incident_views(&state).await?;
    let activity = repo::recent_audit(&state.db, 12).await?;
    Ok(Json(json!({
        "monitors": monitors,
        "incidents": incidents,
        "activity": activity.iter().map(present::AuditResponse::from).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct MonitorBody {
    name: String,
    description: Option<String>,
    job_type: JobType,
    url: String,
    method: Option<String>,
    headers: Option<Vec<HeaderPair>>,
    body: Option<String>,
    timeout_ms: Option<i64>,
    interval_seconds: Option<i64>,
    degraded_after_ms: Option<i64>,
    expected_status: Option<i64>,
    assertions: Option<Vec<Assertion>>,
    follow_redirects: Option<bool>,
    failure_threshold: Option<i32>,
    active: Option<bool>,
}

async fn list_monitors(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Vec<present::MonitorResponse>>, AppError> {
    require_user(&state, &jar).await?;
    let stats = load_stats(&state).await?;
    let monitors = repo::list_monitors(&state.db).await?;
    Ok(Json(
        monitors
            .iter()
            .map(|monitor| present::monitor_response(monitor, stats_view(&stats), 30, Vec::new()))
            .collect(),
    ))
}

async fn create_monitor(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<MonitorBody>,
) -> Result<(StatusCode, Json<present::MonitorResponse>), AppError> {
    let user = require_user(&state, &jar).await?;
    let input = monitor_input(body);
    validate_monitor(&input).map_err(AppError::Validation)?;
    let now = TimestampMillis::now();
    let monitor = materialize(Id::new(), &input, now, None);
    repo::insert_monitor(&state.db, &monitor).await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(user.id),
        "monitor.created",
        "monitor",
        Some(&monitor.id.to_string()),
        now,
    )
    .await;
    let stats = load_stats(&state).await?;
    Ok((
        StatusCode::CREATED,
        Json(present::monitor_response(
            &monitor,
            stats_view(&stats),
            30,
            Vec::new(),
        )),
    ))
}

async fn get_monitor(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<Json<present::MonitorResponse>, AppError> {
    require_user(&state, &jar).await?;
    Ok(Json(detail(&state, id).await?))
}

async fn update_monitor(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
    Json(body): Json<MonitorBody>,
) -> Result<Json<present::MonitorResponse>, AppError> {
    let user = require_user(&state, &jar).await?;
    let existing = repo::get_monitor(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    reject_agent_monitor(&existing, "Edit this check in the agent config.")?;
    let input = monitor_input(body);
    validate_monitor(&input).map_err(AppError::Validation)?;
    let now = TimestampMillis::now();
    let mut monitor = materialize(id, &input, now, Some(&existing));
    if monitor.active && !existing.active {
        monitor.next_check_at = now;
    }
    repo::update_monitor(&state.db, &monitor).await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(user.id),
        "monitor.updated",
        "monitor",
        Some(&id.to_string()),
        now,
    )
    .await;
    Ok(Json(detail(&state, id).await?))
}

async fn delete_monitor(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<StatusCode, AppError> {
    let user = require_user(&state, &jar).await?;
    if !repo::delete_monitor(&state.db, id).await? {
        return Err(AppError::NotFound);
    }
    let _ = repo::insert_audit(
        &state.db,
        Some(user.id),
        "monitor.deleted",
        "monitor",
        Some(&id.to_string()),
        TimestampMillis::now(),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn run_check(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<Json<present::MonitorResponse>, AppError> {
    require_user(&state, &jar).await?;
    let monitor = repo::get_monitor(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    reject_agent_monitor(&monitor, "The agent runs this check on its own schedule.")?;
    engine::execute(&state, id).await?;
    Ok(Json(detail(&state, id).await?))
}

async fn list_agents(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Vec<present::AgentResponse>>, AppError> {
    require_user(&state, &jar).await?;
    let agents = repo::list_agents(&state.db).await?;
    Ok(Json(
        agents.iter().map(present::AgentResponse::from).collect(),
    ))
}

#[derive(Deserialize)]
struct AgentBody {
    name: String,
}

async fn create_agent(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<AgentBody>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let user = require_user(&state, &jar).await?;
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(AppError::field("name", "Use 1 to 60 characters."));
    }
    if repo::agent_by_name(&state.db, name).await?.is_some() {
        return Err(AppError::field(
            "name",
            "An agent with this name already exists.",
        ));
    }
    let id = Id::new();
    let token = generate_token();
    let now = TimestampMillis::now();
    repo::insert_agent(&state.db, id, name, &hash_token(&token), now).await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(user.id),
        "agent.created",
        "agent",
        Some(&id.to_string()),
        now,
    )
    .await;
    let agent = repo::get_agent(&state.db, id)
        .await?
        .ok_or(AppError::Internal)?;
    let mut response = json!(present::AgentResponse::from(&agent));
    response["token"] = json!(token);
    Ok((StatusCode::CREATED, Json(response)))
}

async fn delete_agent(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<StatusCode, AppError> {
    let user = require_user(&state, &jar).await?;
    let agent = repo::get_agent(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if !repo::delete_agent(&state.db, id).await? {
        return Err(AppError::Conflict(format!(
            "{} still has {} monitor(s). Delete them first.",
            agent.name, agent.monitor_count
        )));
    }
    let _ = repo::insert_audit(
        &state.db,
        Some(user.id),
        "agent.deleted",
        "agent",
        Some(&id.to_string()),
        TimestampMillis::now(),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct AgentChecksBody {
    checks: Vec<AgentCheckBody>,
}

#[derive(Deserialize)]
struct AgentCheckBody {
    name: String,
    check_type: CheckType,
    target: String,
    interval_seconds: i64,
    timeout_ms: i64,
    failure_threshold: i32,
}

/// The agent sends its config, without secrets, when it starts.
async fn agent_checks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AgentChecksBody>,
) -> Result<Json<Value>, AppError> {
    let agent = require_agent(&state, &headers).await?;
    let mut checks: Vec<repo::AgentCheck> = Vec::new();
    for check in body.checks {
        let name = check.name.trim().to_owned();
        let input = MonitorInput {
            name: name.clone(),
            description: String::new(),
            job_type: check.check_type.job_type(),
            url: check.target.trim().to_owned(),
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: String::new(),
            timeout_ms: check.timeout_ms,
            interval_seconds: check.interval_seconds,
            degraded_after_ms: None,
            expected_status: None,
            assertions: Vec::new(),
            follow_redirects: true,
            failure_threshold: check.failure_threshold,
            active: true,
        };
        if let Err(fields) = validate_monitor(&input) {
            let detail = fields
                .iter()
                .map(|(field, message)| format!("{field}: {message}"))
                .collect::<Vec<_>>()
                .join(" ");
            return Err(AppError::field(
                "checks",
                format!("Check \"{name}\": {detail}"),
            ));
        }
        if checks.iter().any(|known| known.name == name) {
            return Err(AppError::field(
                "checks",
                format!("Check \"{name}\" is listed twice."),
            ));
        }
        checks.push(repo::AgentCheck {
            name,
            check_type: check.check_type,
            target: input.url,
            interval_seconds: input.interval_seconds,
            timeout_ms: input.timeout_ms,
            failure_threshold: input.failure_threshold,
        });
    }
    let ids = repo::sync_agent_checks(&state.db, agent.id, &checks, TimestampMillis::now()).await?;
    Ok(Json(json!({
        "checks": ids
            .iter()
            .map(|(name, id)| json!({ "name": name, "monitor_id": id }))
            .collect::<Vec<_>>(),
    })))
}

async fn agent_heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    require_agent(&state, &headers).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct AgentResultBody {
    monitor_id: Id,
    #[serde(flatten)]
    outcome: engine::Outcome,
}

async fn agent_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AgentResultBody>,
) -> Result<StatusCode, AppError> {
    let agent = require_agent(&state, &headers).await?;
    let monitor = repo::get_monitor(&state.db, body.monitor_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if monitor.agent_id != Some(agent.id) {
        return Err(AppError::Forbidden(
            "This monitor does not belong to this agent.",
        ));
    }
    engine::submit(&state, monitor.id, body.outcome, &agent.name).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ActiveBody {
    active: bool,
}

async fn set_active(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
    Json(body): Json<ActiveBody>,
) -> Result<Json<present::MonitorResponse>, AppError> {
    require_user(&state, &jar).await?;
    let monitor = repo::get_monitor(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let now = TimestampMillis::now();
    let next = if body.active {
        now.as_millis()
    } else {
        monitor.next_check_at.as_millis()
    };
    repo::set_active(&state.db, id, body.active, next, now.as_millis()).await?;
    Ok(Json(detail(&state, id).await?))
}

#[derive(Deserialize)]
struct PageBody {
    title: String,
    description: Option<String>,
    slug: String,
    published: Option<bool>,
    homepage_url: Option<String>,
    monitor_ids: Vec<Id>,
}

async fn list_pages(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Vec<present::PageResponse>>, AppError> {
    require_user(&state, &jar).await?;
    let pages = repo::list_pages(&state.db).await?;
    let mut response = Vec::new();
    for page in pages {
        let components = repo::components(&state.db, page.id).await?;
        response.push(present::page_response(&page, &components));
    }
    Ok(Json(response))
}

async fn create_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<PageBody>,
) -> Result<(StatusCode, Json<present::PageResponse>), AppError> {
    let user = require_user(&state, &jar).await?;
    let (title, description, slug, homepage) = validate_page(&body)?;
    if repo::page_by_slug(&state.db, &slug).await?.is_some() {
        return Err(AppError::Conflict(
            "That status page slug is already in use.".into(),
        ));
    }
    let names = component_names(&state, &body.monitor_ids).await?;
    let now = TimestampMillis::now();
    let page = Page {
        id: Id::new(),
        title,
        description,
        slug,
        published: body.published.unwrap_or(true),
        homepage_url: homepage,
        created_at: now,
        updated_at: now,
    };
    repo::insert_page(&state.db, &page).await?;
    repo::replace_components(&state.db, page.id, &names, now).await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(user.id),
        "page.created",
        "page",
        Some(&page.id.to_string()),
        now,
    )
    .await;
    let components = repo::components(&state.db, page.id).await?;
    Ok((
        StatusCode::CREATED,
        Json(present::page_response(&page, &components)),
    ))
}

async fn get_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<Json<present::PageResponse>, AppError> {
    require_user(&state, &jar).await?;
    let page = repo::get_page(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let components = repo::components(&state.db, id).await?;
    Ok(Json(present::page_response(&page, &components)))
}

async fn update_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
    Json(body): Json<PageBody>,
) -> Result<Json<present::PageResponse>, AppError> {
    require_user(&state, &jar).await?;
    let existing = repo::get_page(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let (title, description, slug, homepage) = validate_page(&body)?;
    if let Some(other) = repo::page_by_slug(&state.db, &slug).await? {
        if other.id != id {
            return Err(AppError::Conflict(
                "That status page slug is already in use.".into(),
            ));
        }
    }
    let names = component_names(&state, &body.monitor_ids).await?;
    let now = TimestampMillis::now();
    let page = Page {
        id,
        title,
        description,
        slug,
        published: body.published.unwrap_or(existing.published),
        homepage_url: homepage,
        created_at: existing.created_at,
        updated_at: now,
    };
    repo::update_page(&state.db, &page).await?;
    repo::replace_components(&state.db, id, &names, now).await?;
    let components = repo::components(&state.db, id).await?;
    Ok(Json(present::page_response(&page, &components)))
}

async fn delete_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<StatusCode, AppError> {
    require_user(&state, &jar).await?;
    if !repo::delete_page(&state.db, id).await? {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct IncidentBody {
    title: String,
    status: IncidentStatus,
    impact: IncidentImpact,
    message: String,
    page_id: Option<Id>,
    monitor_id: Option<Id>,
}

#[derive(Deserialize)]
struct UpdateBody {
    status: IncidentStatus,
    message: String,
}

async fn list_incidents(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Vec<present::IncidentResponse>>, AppError> {
    require_user(&state, &jar).await?;
    Ok(Json(
        incident_views(&state, repo::list_incidents(&state.db).await?).await?,
    ))
}

async fn create_incident(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<IncidentBody>,
) -> Result<(StatusCode, Json<present::IncidentResponse>), AppError> {
    let user = require_user(&state, &jar).await?;
    let (title, message) = validate_incident(&body.title, &body.message)?;
    ensure_links(&state, body.page_id, body.monitor_id).await?;
    let now = TimestampMillis::now();
    let id = Id::new();
    let incident = Incident {
        id,
        page_id: body.page_id,
        monitor_id: body.monitor_id,
        title,
        status: body.status,
        impact: body.impact,
        auto: false,
        started_at: now,
        resolved_at: resolved_at_after_update(None, body.status, now.as_millis())
            .map(TimestampMillis::from_millis),
        created_at: now,
        updated_at: now,
    };
    let update = IncidentUpdate {
        id: Id::new(),
        incident_id: id,
        status: body.status,
        message: message.clone(),
        created_at: now,
    };
    let mut tx = state.db.begin().await?;
    repo::insert_incident(&mut tx, &incident, &update).await?;
    tx.commit().await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(user.id),
        "incident.created",
        "incident",
        Some(&id.to_string()),
        now,
    )
    .await;
    notify_incident(&state, &incident, &message).await;
    Ok((
        StatusCode::CREATED,
        Json(present::incident_response(&incident, &[update])),
    ))
}

async fn get_incident(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<Json<present::IncidentResponse>, AppError> {
    require_user(&state, &jar).await?;
    let incident = repo::get_incident(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let updates = repo::updates_for(&state.db, &[id]).await?;
    Ok(Json(present::incident_response(&incident, &updates)))
}

async fn add_update(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
    Json(body): Json<UpdateBody>,
) -> Result<Json<present::IncidentResponse>, AppError> {
    let user = require_user(&state, &jar).await?;
    let incident = repo::get_incident(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let message = body.message.trim().to_owned();
    if message.is_empty() || message.chars().count() > 5_000 {
        return Err(AppError::field(
            "message",
            "Write an update between 1 and 5000 characters.",
        ));
    }
    let now = TimestampMillis::now();
    let resolved = resolved_at_after_update(
        incident.resolved_at.map(TimestampMillis::as_millis),
        body.status,
        now.as_millis(),
    );
    let update = IncidentUpdate {
        id: Id::new(),
        incident_id: id,
        status: body.status,
        message: message.clone(),
        created_at: now,
    };
    let mut tx = state.db.begin().await?;
    repo::apply_incident_status(&mut tx, id, body.status, resolved, now.as_millis()).await?;
    repo::insert_update(&mut tx, &update).await?;
    tx.commit().await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(user.id),
        "incident.updated",
        "incident",
        Some(&id.to_string()),
        now,
    )
    .await;
    let updated = repo::get_incident(&state.db, id)
        .await?
        .ok_or(AppError::Internal)?;
    notify_incident(&state, &updated, &message).await;
    let updates = repo::updates_for(&state.db, &[id]).await?;
    Ok(Json(present::incident_response(&updated, &updates)))
}

async fn delete_incident(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<StatusCode, AppError> {
    require_user(&state, &jar).await?;
    let result = sqlx::query("DELETE FROM incidents WHERE id = $1")
        .bind(id.to_string())
        .execute(state.db.pool())
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct MaintenanceBody {
    title: String,
    message: String,
    page_id: Id,
    monitor_ids: Vec<Id>,
    starts_at: TimestampMillis,
    ends_at: TimestampMillis,
}

async fn list_maintenances(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Vec<present::MaintenanceResponse>>, AppError> {
    require_user(&state, &jar).await?;
    let now = TimestampMillis::now();
    let rows = repo::list_maintenances(&state.db).await?;
    Ok(Json(
        rows.iter()
            .map(|row| present::maintenance_response(row, now))
            .collect(),
    ))
}

async fn create_maintenance(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<MaintenanceBody>,
) -> Result<(StatusCode, Json<present::MaintenanceResponse>), AppError> {
    require_user(&state, &jar).await?;
    let title = body.title.trim().to_owned();
    let message = body.message.trim().to_owned();
    let mut fields = BTreeMap::new();
    if title.is_empty() || title.chars().count() > 160 {
        fields.insert("title".to_owned(), "Use 1 to 160 characters.".to_owned());
    }
    if message.is_empty() || message.chars().count() > 5_000 {
        fields.insert(
            "message".to_owned(),
            "Describe the window in 1 to 5000 characters.".to_owned(),
        );
    }
    if body.ends_at.as_millis() <= body.starts_at.as_millis() {
        fields.insert(
            "ends_at".to_owned(),
            "The window must end after it starts.".to_owned(),
        );
    }
    if !fields.is_empty() {
        return Err(AppError::Validation(fields));
    }
    if repo::get_page(&state.db, body.page_id).await?.is_none() {
        return Err(AppError::field("page_id", "Choose a status page."));
    }
    for monitor_id in &body.monitor_ids {
        if repo::get_monitor(&state.db, *monitor_id).await?.is_none() {
            return Err(AppError::field(
                "monitor_ids",
                "One of the monitors does not exist.",
            ));
        }
    }
    let now = TimestampMillis::now();
    let maintenance = Maintenance {
        id: Id::new(),
        page_id: Some(body.page_id),
        title,
        message,
        starts_at: body.starts_at,
        ends_at: body.ends_at,
        monitor_ids: body.monitor_ids,
        created_at: now,
    };
    repo::insert_maintenance(&state.db, &maintenance).await?;
    Ok((
        StatusCode::CREATED,
        Json(present::maintenance_response(&maintenance, now)),
    ))
}

async fn delete_maintenance(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<StatusCode, AppError> {
    require_user(&state, &jar).await?;
    if !repo::delete_maintenance(&state.db, id).await? {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ChannelBody {
    name: String,
    provider: String,
    config: Value,
    enabled: Option<bool>,
    monitor_ids: Vec<Id>,
}

async fn list_channels(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Vec<present::ChannelResponse>>, AppError> {
    require_user(&state, &jar).await?;
    let channels = repo::list_channels(&state.db).await?;
    let mut response = Vec::new();
    for channel in channels {
        response.push(channel_view(&state, channel).await?);
    }
    Ok(Json(response))
}

async fn create_channel(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<ChannelBody>,
) -> Result<(StatusCode, Json<present::ChannelResponse>), AppError> {
    require_user(&state, &jar).await?;
    let (name, provider, config) = validate_channel(&body)?;
    ensure_monitors(&state, &body.monitor_ids).await?;
    let now = TimestampMillis::now();
    let channel = Channel {
        id: Id::new(),
        name,
        provider,
        config_json: config,
        enabled: body.enabled.unwrap_or(true),
        created_at: now,
    };
    repo::insert_channel(&state.db, &channel, &body.monitor_ids).await?;
    Ok((
        StatusCode::CREATED,
        Json(channel_view(&state, channel).await?),
    ))
}

async fn update_channel(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
    Json(body): Json<ChannelBody>,
) -> Result<Json<present::ChannelResponse>, AppError> {
    require_user(&state, &jar).await?;
    let existing = repo::get_channel(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let (name, provider, config) = validate_channel(&body)?;
    ensure_monitors(&state, &body.monitor_ids).await?;
    let channel = Channel {
        id,
        name,
        provider,
        config_json: config,
        enabled: body.enabled.unwrap_or(existing.enabled),
        created_at: existing.created_at,
    };
    repo::update_channel(
        &state.db,
        &channel,
        &body.monitor_ids,
        TimestampMillis::now(),
    )
    .await?;
    Ok(Json(channel_view(&state, channel).await?))
}

async fn delete_channel(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<StatusCode, AppError> {
    require_user(&state, &jar).await?;
    if !repo::delete_channel(&state.db, id).await? {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn test_channel(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<Id>,
) -> Result<Json<present::ChannelResponse>, AppError> {
    require_user(&state, &jar).await?;
    let channel = repo::get_channel(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let payload = NotificationPayload {
        event: NotificationEvent::Test.as_str().to_owned(),
        sent_at: TimestampMillis::now().to_string(),
        monitor: None,
        check: None,
        incident: None,
        message: Some("This is a test notification from Owlee.".to_owned()),
    };
    notify::deliver(&state, &channel, &payload, None, None).await;
    Ok(Json(channel_view(&state, channel).await?))
}

async fn public_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<present::PublicPage>, AppError> {
    let page = repo::page_by_slug(&state.db, &slug)
        .await?
        .ok_or(AppError::NotFound)?;
    if !page.published {
        return Err(AppError::NotFound);
    }
    let components = repo::components(&state.db, page.id).await?;
    let stats = load_stats(&state).await?;
    let monitors = repo::list_monitors(&state.db).await?;
    let mut public_components = Vec::new();
    let mut states = Vec::new();
    for component in &components {
        let Some(monitor) = monitors
            .iter()
            .find(|monitor| monitor.id == component.monitor_id)
        else {
            continue;
        };
        states.push(monitor.state);
        let response = present::monitor_response(monitor, stats_view(&stats), 90, Vec::new());
        public_components.push(present::PublicComponent {
            id: component.id,
            name: component.name.clone(),
            description: if component.description.is_empty() {
                monitor.description.clone()
            } else {
                component.description.clone()
            },
            state: monitor.state,
            uptime_90d: response.uptime_90d,
            bars: response.bars,
        });
    }
    let incidents =
        incident_views(&state, repo::incidents_for_page(&state.db, page.id).await?).await?;
    let open = incidents
        .iter()
        .filter(|incident| incident.status != IncidentStatus::Resolved)
        .map(|incident| OpenIncident {
            impact: incident.impact,
        })
        .collect::<Vec<_>>();
    let banner = page_banner(&states, &open);
    let now = TimestampMillis::now();
    let maintenances = repo::maintenances_for_page(&state.db, page.id)
        .await?
        .iter()
        .map(|maintenance| present::maintenance_response(maintenance, now))
        .filter(|maintenance| maintenance.ends_at.as_millis() >= now.as_millis() - DAY_MS)
        .collect();
    Ok(Json(present::PublicPage {
        title: page.title,
        description: page.description,
        slug: page.slug,
        homepage_url: page.homepage_url,
        banner: present::banner_label(banner),
        components: public_components,
        incidents,
        maintenances,
    }))
}

struct LoadedStats {
    daily: Vec<repo::DailyRow>,
    counts: Vec<repo::StatusCount>,
}

fn stats_view(stats: &LoadedStats) -> Stats<'_> {
    Stats {
        daily: &stats.daily,
        counts: &stats.counts,
    }
}

async fn load_stats(state: &AppState) -> Result<LoadedStats, AppError> {
    let now = TimestampMillis::now();
    Ok(LoadedStats {
        daily: repo::daily_since(&state.db, now.unix_day() - 89).await?,
        counts: repo::counts_since(&state.db, now.as_millis() - DAY_MS).await?,
    })
}

async fn detail(state: &AppState, id: Id) -> Result<present::MonitorResponse, AppError> {
    let monitor = repo::get_monitor(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let stats = load_stats(state).await?;
    let recent = repo::recent_checks(&state.db, id, 60).await?;
    Ok(present::monitor_response(
        &monitor,
        stats_view(&stats),
        90,
        recent,
    ))
}

async fn open_incident_views(state: &AppState) -> Result<Vec<present::IncidentResponse>, AppError> {
    let incidents = repo::list_incidents(&state.db)
        .await?
        .into_iter()
        .filter(|incident| incident.status != IncidentStatus::Resolved)
        .take(8)
        .collect::<Vec<_>>();
    incident_views(state, incidents).await
}

async fn incident_views(
    state: &AppState,
    incidents: Vec<Incident>,
) -> Result<Vec<present::IncidentResponse>, AppError> {
    let ids = incidents
        .iter()
        .map(|incident| incident.id)
        .collect::<Vec<_>>();
    let updates = repo::updates_for(&state.db, &ids).await?;
    Ok(incidents
        .iter()
        .map(|incident| present::incident_response(incident, &updates))
        .collect())
}

async fn notify_incident(state: &AppState, incident: &Incident, message: &str) {
    let monitor = match incident.monitor_id {
        Some(id) => repo::get_monitor(&state.db, id).await.ok().flatten(),
        None => None,
    };
    let payload = NotificationPayload {
        event: NotificationEvent::IncidentUpdated.as_str().to_owned(),
        sent_at: TimestampMillis::now().to_string(),
        monitor: monitor.map(|monitor| owlee_domain::PayloadMonitor {
            id: monitor.id.to_string(),
            name: monitor.name,
            url: monitor.url,
            state: monitor.state,
        }),
        check: None,
        incident: Some(owlee_domain::PayloadIncident {
            id: incident.id.to_string(),
            title: incident.title.clone(),
            status: incident.status,
            impact: incident.impact,
        }),
        message: Some(message.to_owned()),
    };
    notify::fanout(state, &payload, incident.monitor_id, Some(incident.id)).await;
}

fn monitor_input(body: MonitorBody) -> MonitorInput {
    MonitorInput {
        name: body.name,
        description: body.description.unwrap_or_default(),
        job_type: body.job_type,
        url: body.url.trim().to_owned(),
        method: body.method.unwrap_or_else(|| "GET".to_owned()),
        headers: body.headers.unwrap_or_default(),
        body: body.body.unwrap_or_default(),
        timeout_ms: body.timeout_ms.unwrap_or(30_000),
        interval_seconds: body.interval_seconds.unwrap_or(60),
        degraded_after_ms: body.degraded_after_ms,
        expected_status: body.expected_status,
        assertions: body.assertions.unwrap_or_default(),
        follow_redirects: body.follow_redirects.unwrap_or(true),
        failure_threshold: body.failure_threshold.unwrap_or(3),
        active: body.active.unwrap_or(true),
    }
}

fn materialize(
    id: Id,
    input: &MonitorInput,
    now: TimestampMillis,
    previous: Option<&Monitor>,
) -> Monitor {
    Monitor {
        id,
        name: input.name.trim().to_owned(),
        description: input.description.trim().to_owned(),
        job_type: input.job_type,
        url: input.url.clone(),
        method: input.method.clone(),
        headers: input.headers.clone(),
        body: input.body.clone(),
        timeout_ms: input.timeout_ms,
        interval_seconds: input.interval_seconds,
        degraded_after_ms: input.degraded_after_ms,
        expected_status: input.expected_status,
        assertions: input.assertions.clone(),
        follow_redirects: input.follow_redirects,
        failure_threshold: input.failure_threshold,
        active: input.active,
        state: previous
            .map(|monitor| monitor.state)
            .unwrap_or(MonitorState::Pending),
        consecutive_failures: previous
            .map(|monitor| monitor.consecutive_failures)
            .unwrap_or(0),
        last_checked_at: previous.and_then(|monitor| monitor.last_checked_at),
        next_check_at: previous.map(|monitor| monitor.next_check_at).unwrap_or(now),
        created_at: previous.map(|monitor| monitor.created_at).unwrap_or(now),
        updated_at: now,
        agent_id: None,
        check_type: None,
    }
}

fn validate_page(body: &PageBody) -> Result<(String, String, String, Option<String>), AppError> {
    let mut fields = BTreeMap::new();
    let title = body.title.trim().to_owned();
    if title.is_empty() || title.chars().count() > 120 {
        fields.insert("title".to_owned(), "Use 1 to 120 characters.".to_owned());
    }
    let description = body.description.clone().unwrap_or_default();
    if description.chars().count() > 2_000 {
        fields.insert(
            "description".to_owned(),
            "Keep the description under 2000 characters.".to_owned(),
        );
    }
    let slug = body.slug.trim().to_lowercase();
    if validate_slug(&slug).is_err() {
        fields.insert(
            "slug".to_owned(),
            "Use 1 to 64 lowercase letters, numbers, and single hyphens.".to_owned(),
        );
    }
    let homepage = body
        .homepage_url
        .clone()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(url) = &homepage {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            fields.insert(
                "homepage_url".to_owned(),
                "Homepage must be an http(s) URL.".to_owned(),
            );
        }
    }
    if fields.is_empty() {
        Ok((title, description.trim().to_owned(), slug, homepage))
    } else {
        Err(AppError::Validation(fields))
    }
}

fn validate_incident(title: &str, message: &str) -> Result<(String, String), AppError> {
    let mut fields = BTreeMap::new();
    let title = title.trim().to_owned();
    let message = message.trim().to_owned();
    if title.is_empty() || title.chars().count() > 200 {
        fields.insert("title".to_owned(), "Use 1 to 200 characters.".to_owned());
    }
    if message.is_empty() || message.chars().count() > 5_000 {
        fields.insert(
            "message".to_owned(),
            "Write an update between 1 and 5000 characters.".to_owned(),
        );
    }
    if fields.is_empty() {
        Ok((title, message))
    } else {
        Err(AppError::Validation(fields))
    }
}

fn validate_channel(body: &ChannelBody) -> Result<(String, String, String), AppError> {
    let name = body.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::field("name", "Use 1 to 80 characters."));
    }
    let provider = body.provider.trim().to_lowercase();
    let config = match provider.as_str() {
        "email" => {
            let to = body
                .config
                .get("to")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if !valid_email(to) {
                return Err(AppError::field("config", "Enter the recipient email."));
            }
            json!({ "to": to }).to_string()
        }
        "webhook" => {
            let url = require_http_url(body.config.get("url"))?;
            let secret = body
                .config
                .get("secret")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            json!({ "url": url, "secret": secret }).to_string()
        }
        "slack" | "discord" => {
            let url = require_http_url(body.config.get("url"))?;
            json!({ "url": url }).to_string()
        }
        _ => {
            return Err(AppError::field(
                "provider",
                "Use email, webhook, slack, or discord.",
            ));
        }
    };
    Ok((name, provider, config))
}

fn require_http_url(value: Option<&Value>) -> Result<String, AppError> {
    let url = value
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    if url.starts_with("https://") || url.starts_with("http://") {
        Ok(url)
    } else {
        Err(AppError::field("config", "Enter an http(s) URL."))
    }
}

async fn component_names(state: &AppState, ids: &[Id]) -> Result<Vec<(Id, String)>, AppError> {
    let mut names = Vec::new();
    for id in ids {
        let monitor = repo::get_monitor(&state.db, *id)
            .await?
            .ok_or_else(|| AppError::field("monitor_ids", "One of the monitors does not exist."))?;
        names.push((monitor.id, monitor.name));
    }
    Ok(names)
}

async fn ensure_links(
    state: &AppState,
    page_id: Option<Id>,
    monitor_id: Option<Id>,
) -> Result<(), AppError> {
    if let Some(page_id) = page_id {
        if repo::get_page(&state.db, page_id).await?.is_none() {
            return Err(AppError::field("page_id", "Choose a status page."));
        }
    }
    if let Some(monitor_id) = monitor_id {
        if repo::get_monitor(&state.db, monitor_id).await?.is_none() {
            return Err(AppError::field("monitor_id", "Choose a monitor."));
        }
    }
    Ok(())
}

async fn ensure_monitors(state: &AppState, ids: &[Id]) -> Result<(), AppError> {
    for id in ids {
        if repo::get_monitor(&state.db, *id).await?.is_none() {
            return Err(AppError::field(
                "monitor_ids",
                "One of the monitors does not exist.",
            ));
        }
    }
    Ok(())
}

async fn channel_view(
    state: &AppState,
    channel: Channel,
) -> Result<present::ChannelResponse, AppError> {
    let monitors = repo::channel_monitors(&state.db, channel.id).await?;
    let deliveries = repo::recent_deliveries(&state.db, channel.id).await?;
    Ok(present::channel_response(&channel, monitors, deliveries))
}

fn reject_agent_monitor(monitor: &Monitor, reason: &str) -> Result<(), AppError> {
    match monitor.agent_id {
        Some(_) => Err(AppError::Conflict(reason.to_owned())),
        None => Ok(()),
    }
}

async fn require_agent(state: &AppState, headers: &HeaderMap) -> Result<Agent, AppError> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;
    repo::authenticate_agent(&state.db, &hash_token(token.trim()), TimestampMillis::now())
        .await?
        .ok_or(AppError::Unauthorized)
}

async fn user_response(state: &AppState, user: &User) -> Result<present::UserResponse, AppError> {
    let root_id = instance_settings::root_user_id(&state.db).await?;
    Ok(present::UserResponse::with_access(user, root_id))
}

async fn require_root(state: &AppState, jar: &CookieJar) -> Result<User, AppError> {
    let user = require_user(state, jar).await?;
    let root_id = instance_settings::root_user_id(&state.db).await?;
    if root_id != Some(user.id) {
        return Err(AppError::Forbidden(
            "Only the root user can change instance settings.",
        ));
    }
    Ok(user)
}

async fn require_installation_admin(state: &AppState, jar: &CookieJar) -> Result<User, AppError> {
    let user = require_user(state, jar).await?;
    let root_id = instance_settings::root_user_id(&state.db).await?;
    let admin = root_id == Some(user.id) || user.role == "owner" || user.role == "admin";
    if !admin {
        return Err(AppError::Forbidden(
            "Only an installation admin can view accounts.",
        ));
    }
    Ok(user)
}

pub(crate) async fn require_user(state: &AppState, jar: &CookieJar) -> Result<User, AppError> {
    let Some(token) = session_cookie_names()
        .find_map(|name| jar.get(name).map(|cookie| cookie.value().to_owned()))
    else {
        return Err(AppError::Unauthorized);
    };
    repo::authenticate(&state.db, &hash_token(&token), TimestampMillis::now())
        .await?
        .ok_or(AppError::Unauthorized)
}

async fn issue_session(
    state: &AppState,
    jar: CookieJar,
    user: &User,
    now: TimestampMillis,
) -> Result<CookieJar, AppError> {
    let token = generate_token();
    let idle = TimestampMillis::from_millis(now.as_millis() + 30 * DAY_MS);
    let absolute = TimestampMillis::from_millis(now.as_millis() + 90 * DAY_MS);
    repo::insert_session(
        &state.db,
        Id::new(),
        &hash_token(&token),
        user.id,
        now,
        idle,
        absolute,
    )
    .await?;
    Ok(session_jar(state, jar, token))
}

fn session_jar(state: &AppState, jar: CookieJar, token: String) -> CookieJar {
    let cookie = Cookie::build((COOKIE, token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .secure(state.config.secure_cookies())
        .build();
    let mut jar = jar.add(cookie);
    for name in LEGACY_COOKIES {
        let mut legacy = Cookie::from(*name);
        legacy.set_path("/");
        jar = jar.remove(legacy);
    }
    jar
}

struct AccountFields {
    email: String,
    normalized: String,
    display_name: String,
}

fn account_fields(email: &str, display_name: Option<String>) -> Result<AccountFields, AppError> {
    let email = email.trim();
    let normalized = normalize_email(email);
    if !valid_email(&normalized) {
        return Err(AppError::field("email", "Enter a valid email address."));
    }
    let display_name = display_name.unwrap_or_default().trim().to_owned();
    let display_name = if display_name.is_empty() {
        normalized
            .split('@')
            .next()
            .unwrap_or("Operator")
            .to_owned()
    } else if display_name.chars().count() > 80 {
        return Err(AppError::field(
            "display_name",
            "Use at most 80 characters.",
        ));
    } else {
        display_name
    };
    Ok(AccountFields {
        email: email.to_owned(),
        normalized,
        display_name,
    })
}

fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !email.contains(char::is_whitespace)
        && email.len() <= 200
}

fn password_field(error: PasswordError) -> AppError {
    AppError::field("password", error.to_string())
}
