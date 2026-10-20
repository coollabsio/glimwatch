use std::collections::BTreeMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use status_platform::Problem;

#[derive(Debug)]
pub enum AppError {
    Unauthorized,
    InvalidLogin,
    Forbidden(&'static str),
    NotFound,
    Busy,
    /// The Status host has no network, so a failed check proves nothing.
    Offline,
    Conflict(String),
    Validation(BTreeMap<String, String>),
    Internal,
}

impl AppError {
    pub fn field(name: &str, message: impl Into<String>) -> Self {
        let mut fields = BTreeMap::new();
        fields.insert(name.to_owned(), message.into());
        Self::Validation(fields)
    }
}

impl std::error::Error for AppError {}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => formatter.write_str("unauthorized"),
            Self::InvalidLogin => formatter.write_str("invalid login"),
            Self::Forbidden(detail) => write!(formatter, "forbidden: {detail}"),
            Self::NotFound => formatter.write_str("not found"),
            Self::Busy => formatter.write_str("check already running"),
            Self::Offline => formatter.write_str("status host has no network"),
            Self::Conflict(detail) => write!(formatter, "conflict: {detail}"),
            Self::Validation(_) => formatter.write_str("validation failed"),
            Self::Internal => formatter.write_str("internal error"),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!(%error, "database error");
        Self::Internal
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let problem = match self {
            Self::Unauthorized => Problem::unauthorized(),
            Self::InvalidLogin => Problem {
                type_uri: "https://status.dev/problems/invalid-login".to_owned(),
                title: "Sign-in failed".to_owned(),
                status: 401,
                code: "invalid_login".to_owned(),
                detail: "Invalid email or password.".to_owned(),
                fields: None,
            },
            Self::Forbidden(detail) => Problem {
                type_uri: "https://status.dev/problems/forbidden".to_owned(),
                title: "Forbidden".to_owned(),
                status: 403,
                code: "forbidden".to_owned(),
                detail: detail.to_owned(),
                fields: None,
            },
            Self::NotFound => Problem::not_found(),
            Self::Busy => Problem::conflict("A check is already running for this monitor."),
            Self::Offline => Problem {
                type_uri: "https://status.dev/problems/host-offline".to_owned(),
                title: "Status host has no network".to_owned(),
                status: 503,
                code: "host_offline".to_owned(),
                detail: "The check failed, but Status cannot reach any connectivity target either. The result was not recorded.".to_owned(),
                fields: None,
            },
            Self::Conflict(detail) => Problem::conflict(detail),
            Self::Validation(fields) => Problem::validation(fields),
            Self::Internal => Problem::internal(),
        };
        let status =
            StatusCode::from_u16(problem.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(problem)).into_response()
    }
}
