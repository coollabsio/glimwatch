use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, Method, Response, StatusCode, Uri, header};
use axum::response::IntoResponse;
use include_dir::{Dir, include_dir};

use crate::state::AppState;

static DIST: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../web/dist");

pub async fn spa(State(_state): State<AppState>, method: Method, uri: Uri) -> Response<Body> {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let path = uri.path().trim_start_matches('/');
    if path.contains("..") {
        return StatusCode::NOT_FOUND.into_response();
    }
    if path.is_empty() {
        return index();
    }
    if let Some(file) = DIST.get_file(path) {
        return file_response(path, file.contents(), path.starts_with("assets/"));
    }
    if path.contains('.') {
        return StatusCode::NOT_FOUND.into_response();
    }
    index()
}

fn index() -> Response<Body> {
    match DIST.get_file("index.html") {
        Some(file) => file_response("index.html", file.contents(), false),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn file_response(path: &str, contents: &'static [u8], immutable: bool) -> Response<Body> {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if immutable {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, cache)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        )
        .body(Body::from(contents))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
