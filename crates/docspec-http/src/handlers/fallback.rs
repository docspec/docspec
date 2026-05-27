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
pub async fn not_found(uri: Uri, method: Method) -> HttpError {
    HttpError::NotFound {
        path: format!("{method} {}", uri.path()),
    }
}

/// Fallback handler for wrong HTTP methods on `/conversion`.
///
/// Returns 405 Method Not Allowed with `Allow: POST`.
#[inline]
#[must_use]
// Reason: Axum handlers must be async to satisfy the Handler trait, even if they don't await.
#[allow(clippy::unused_async)]
pub async fn conversion_method_not_allowed() -> HttpError {
    HttpError::MethodNotAllowed { allowed: "POST" }
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

#[cfg(test)]
mod tests {
    // Reason: test code legitimately panics on assertion failures; unwrap, expect,
    // and slice indexing are standard testing patterns that express expected outcomes.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

    use axum::{
        body,
        http::{header::CONTENT_TYPE, Request, StatusCode},
        response::IntoResponse as _,
        Router,
    };
    use tower::ServiceExt as _;

    use super::*;

    fn fallback_router() -> Router {
        Router::new().fallback(not_found)
    }

    #[tokio::test]
    async fn unknown_returns_404() {
        let app = fallback_router();
        let request = Request::builder()
            .method("GET")
            .uri("/unknown/path")
            .body(body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let ct = response.headers().get(CONTENT_TYPE).unwrap();
        assert_eq!(ct, "application/problem+json; charset=utf-8");

        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 4, "expected exactly 4 RFC 7807 fields");
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("title"));
        assert!(obj.contains_key("status"));
        assert!(obj.contains_key("detail"));
    }

    #[tokio::test]
    async fn query_string_not_leaked() {
        let app = fallback_router();
        let request = Request::builder()
            .method("GET")
            .uri("/unknown?secret=password")
            .body(body::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let detail = json["detail"].as_str().unwrap();

        assert!(
            !detail.contains("secret"),
            "query param 'secret' leaked into detail: {detail}"
        );
        assert!(
            !detail.contains("password"),
            "query param 'password' leaked into detail: {detail}"
        );
    }

    #[tokio::test]
    async fn conversion_405_has_allow_post() {
        let response = conversion_method_not_allowed().await.into_response();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers().get("allow").unwrap(), "POST");
    }

    #[tokio::test]
    async fn health_405_has_allow_get_head_options() {
        let response = health_method_not_allowed().await.into_response();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            response.headers().get("allow").unwrap(),
            "GET, HEAD, OPTIONS"
        );
    }
}
