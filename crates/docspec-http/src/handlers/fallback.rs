//! Fallback handlers for unmatched routes and disallowed methods.

use axum::http::{Method, Uri};

use crate::error::HttpError;

/// Fallback handler that returns RFC 7807 Problem JSON for all unmatched routes.
///
/// Uses only [`Uri::path`] (not the full URI) to avoid leaking sensitive query
/// parameters in error responses.
#[inline]
#[must_use]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn not_found(request_uri: Uri, method: Method) -> HttpError {
    HttpError::NotFound {
        method: method.to_string(),
        path: request_uri.path().to_owned(),
    }
}

/// Fallback handler for wrong HTTP methods on `/conversion`.
///
/// Returns 405 Method Not Allowed with `Allow: POST, OPTIONS`.
#[inline]
#[must_use]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn conversion_method_not_allowed() -> HttpError {
    HttpError::MethodNotAllowed {
        allowed: "POST, OPTIONS",
    }
}

/// Fallback handler for wrong HTTP methods on `/health`.
///
/// Returns 405 Method Not Allowed with `Allow: GET, HEAD, OPTIONS`.
#[inline]
#[must_use]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn health_method_not_allowed() -> HttpError {
    HttpError::MethodNotAllowed {
        allowed: "GET, HEAD, OPTIONS",
    }
}
