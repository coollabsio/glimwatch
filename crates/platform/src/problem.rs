use std::collections::BTreeMap;

use serde::Serialize;

/// An RFC 9457 problem body. Glimwatch uses the same shape Orbit does so
/// clients can branch on `code` instead of parsing prose.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Problem {
    #[serde(rename = "type")]
    pub type_uri: String,
    pub title: String,
    pub status: u16,
    pub code: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<BTreeMap<String, String>>,
}

impl Problem {
    #[must_use]
    pub fn unauthorized() -> Self {
        Self {
            type_uri: "https://glimwatch.io/problems/unauthorized".to_owned(),
            title: "Unauthorized".to_owned(),
            status: 401,
            code: "unauthorized".to_owned(),
            detail: "Sign in to continue.".to_owned(),
            fields: None,
        }
    }

    #[must_use]
    pub fn not_found() -> Self {
        Self {
            type_uri: "https://glimwatch.io/problems/not-found".to_owned(),
            title: "Not found".to_owned(),
            status: 404,
            code: "not_found".to_owned(),
            detail: "That resource does not exist.".to_owned(),
            fields: None,
        }
    }

    #[must_use]
    pub fn conflict(detail: impl Into<String>) -> Self {
        Self {
            type_uri: "https://glimwatch.io/problems/conflict".to_owned(),
            title: "Conflict".to_owned(),
            status: 409,
            code: "conflict".to_owned(),
            detail: detail.into(),
            fields: None,
        }
    }

    #[must_use]
    pub fn validation(fields: BTreeMap<String, String>) -> Self {
        Self {
            type_uri: "https://glimwatch.io/problems/validation-failed".to_owned(),
            title: "Validation failed".to_owned(),
            status: 422,
            code: "validation_failed".to_owned(),
            detail: "One or more fields are invalid.".to_owned(),
            fields: Some(fields),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self {
            type_uri: "https://glimwatch.io/problems/internal-error".to_owned(),
            title: "Internal server error".to_owned(),
            status: 500,
            code: "internal_error".to_owned(),
            detail: "An unexpected error occurred.".to_owned(),
            fields: None,
        }
    }
}
