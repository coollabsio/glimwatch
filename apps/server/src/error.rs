use std::collections::BTreeMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use statixo_platform::Problem;

#[derive(Debug)]
pub enum AppError {
    Unauthorized,
    InvalidLogin,
    Forbidden(&'static str),
    NotFound,
    Busy,
    /// The Statixo host has no network, so a failed check proves nothing.
    Offline,
    Conflict(String),
    InvalidSetupToken,
    SetupUnavailable,
    RegistrationClosed,
    BadRequest(String),
    Validation(BTreeMap<String, String>),
    TooManyRequests,
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
            Self::InvalidSetupToken => formatter.write_str("invalid setup token"),
            Self::SetupUnavailable => formatter.write_str("setup unavailable"),
            Self::RegistrationClosed => formatter.write_str("registration closed"),
            Self::BadRequest(detail) => write!(formatter, "bad request: {detail}"),
            Self::Validation(_) => formatter.write_str("validation failed"),
            Self::TooManyRequests => formatter.write_str("too many requests"),
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
                type_uri: "https://statixo.io/problems/invalid-login".to_owned(),
                title: "Sign-in failed".to_owned(),
                status: 401,
                code: "invalid_login".to_owned(),
                detail: "Invalid email or password.".to_owned(),
                fields: None,
            },
            Self::Forbidden(detail) => Problem {
                type_uri: "https://statixo.io/problems/forbidden".to_owned(),
                title: "Forbidden".to_owned(),
                status: 403,
                code: "forbidden".to_owned(),
                detail: detail.to_owned(),
                fields: None,
            },
            Self::NotFound => Problem::not_found(),
            Self::TooManyRequests => Problem {
                type_uri: "https://statixo.io/problems/too-many-requests".to_owned(),
                title: "Too many requests".to_owned(),
                status: 429,
                code: "too_many_requests".to_owned(),
                detail: "Slow down and try again.".to_owned(),
                fields: None,
            },
            Self::Busy => Problem::conflict("A check is already running for this monitor."),
            Self::Offline => Problem {
                type_uri: "https://statixo.io/problems/host-offline".to_owned(),
                title: "Statixo host has no network".to_owned(),
                status: 503,
                code: "host_offline".to_owned(),
                detail: "The check failed, but Statixo cannot reach any connectivity target either. The result was not recorded.".to_owned(),
                fields: None,
            },
            Self::Conflict(detail) => Problem::conflict(detail),
            Self::InvalidSetupToken => Problem {
                type_uri: "https://statixo.io/problems/invalid-setup-token".to_owned(),
                title: "Invalid setup token".to_owned(),
                status: 401,
                code: "invalid_setup_token".to_owned(),
                detail: "The setup token is invalid or expired.".to_owned(),
                fields: None,
            },
            Self::SetupUnavailable => Problem {
                type_uri: "https://statixo.io/problems/setup-unavailable".to_owned(),
                title: "Setup unavailable".to_owned(),
                status: 409,
                code: "setup_unavailable".to_owned(),
                detail: "This installation has already been initialized.".to_owned(),
                fields: None,
            },
            Self::RegistrationClosed => Problem {
                type_uri: "https://statixo.io/problems/registration-closed".to_owned(),
                title: "Registration is closed".to_owned(),
                status: 403,
                code: "registration_closed".to_owned(),
                detail: "This Statixo does not accept public sign-up.".to_owned(),
                fields: None,
            },
            Self::BadRequest(detail) => Problem {
                type_uri: "https://statixo.io/problems/bad-request".to_owned(),
                title: "Bad request".to_owned(),
                status: 400,
                code: "bad_request".to_owned(),
                detail,
                fields: None,
            },
            Self::Validation(fields) => Problem::validation(fields),
            Self::Internal => Problem::internal(),
        };
        let status =
            StatusCode::from_u16(problem.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(problem)).into_response()
    }
}
