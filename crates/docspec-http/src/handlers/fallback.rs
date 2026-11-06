//! Fallback handlers for unmatched routes and disallowed methods.

use axum::http::{Method, Uri};

use crate::error::HttpError;
use crate::format;

/// Fallback handler that returns RFC 9457 Problem JSON for all unmatched routes.
///
/// Uses only [`Uri::path`] (not the full URI) to avoid leaking sensitive query
/// parameters in error responses.
#[inline]
#[must_use]
pub async fn not_found(request_uri: Uri, method: Method) -> HttpError {
    HttpError::NotFound {
        method: method.to_string(),
        path: request_uri.path().to_owned(),
    }
}

/// Fallback handler for wrong HTTP methods on `/conversion`.
///
/// Returns 405 Method Not Allowed with `Allow: GET, HEAD, POST, OPTIONS`.
#[inline]
#[must_use]
pub async fn conversion_method_not_allowed() -> HttpError {
    HttpError::MethodNotAllowed {
        allowed: format::ALLOW_CONVERSION,
    }
}

/// Fallback handler for wrong HTTP methods on `/health`.
///
/// Returns 405 Method Not Allowed with `Allow: GET, HEAD, OPTIONS`.
#[inline]
#[must_use]
pub async fn health_method_not_allowed() -> HttpError {
    HttpError::MethodNotAllowed {
        allowed: format::ALLOW_HEALTH,
    }
}

/// Fallback handler for wrong HTTP methods on read-only resources
/// (`/.well-known/docspec`, `/.well-known/api-catalog`, `/openapi.json`).
///
/// Returns 405 Method Not Allowed with `Allow: GET, HEAD`.
#[inline]
#[must_use]
pub async fn read_only_method_not_allowed() -> HttpError {
    HttpError::MethodNotAllowed {
        allowed: format::ALLOW_GET_HEAD,
    }
}
