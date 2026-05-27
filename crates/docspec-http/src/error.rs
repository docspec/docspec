//! HTTP error types and RFC 7807 problem+json responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Result type alias for HTTP handler operations.
pub type Result<T> = core::result::Result<T, HttpError>;

/// HTTP API error variants.
///
/// Each variant maps to a specific HTTP status code and RFC 7807 problem+json response.
#[derive(Debug)]
pub enum HttpError {
    /// The request body is malformed (HTTP 400).
    BadRequest {
        /// Human-readable description of the problem.
        detail: String,
    },
    /// Document conversion failed (HTTP 422).
    Conversion(docspec_core::Error),
    /// An unexpected internal error occurred (HTTP 500).
    Internal {
        /// Human-readable description of the internal error.
        detail: String,
    },
    /// The requested output format is not acceptable (HTTP 406).
    NotAcceptable {
        /// The received Accept header value, if any.
        received: Option<String>,
    },
    /// The requested resource was not found (HTTP 404).
    NotFound,
    /// The request Content-Type is not supported (HTTP 415).
    UnsupportedMediaType {
        /// The received Content-Type value, if any.
        received: Option<String>,
    },
}

/// RFC 7807 Problem Details object for JSON serialization.
#[derive(Serialize)]
struct ProblemJson {
    /// Human-readable explanation specific to this occurrence.
    detail: String,
    /// HTTP status code.
    status: u16,
    /// Short human-readable summary of the problem type.
    title: String,
    /// URI reference identifying the problem type.
    #[serde(rename = "type")]
    type_: String,
}

impl core::fmt::Display for HttpError {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadRequest { detail } => write!(f, "bad request: {detail}"),
            Self::Conversion(err) => write!(f, "conversion failed: {err}"),
            Self::Internal { detail } => write!(f, "internal error: {detail}"),
            Self::NotAcceptable { received } => {
                if let Some(accept) = received {
                    write!(f, "not acceptable: {accept}")
                } else {
                    write!(f, "not acceptable")
                }
            }
            Self::NotFound => write!(f, "not found"),
            Self::UnsupportedMediaType { received } => {
                if let Some(ct) = received {
                    write!(f, "unsupported media type: {ct}")
                } else {
                    write!(f, "unsupported media type: Content-Type header missing")
                }
            }
        }
    }
}

impl core::error::Error for HttpError {
    #[inline]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Conversion(err) => Some(err),
            Self::BadRequest { .. }
            | Self::Internal { .. }
            | Self::NotAcceptable { .. }
            | Self::NotFound
            | Self::UnsupportedMediaType { .. } => None,
        }
    }
}

impl IntoResponse for HttpError {
    #[inline]
    fn into_response(self) -> Response {
        let (status, code, title, detail) = match &self {
            Self::BadRequest { detail } => (
                StatusCode::BAD_REQUEST,
                "bad-request",
                "Bad Request",
                detail.clone(),
            ),
            Self::Conversion(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "conversion-failed",
                "Conversion Failed",
                "The document could not be converted. Check that the input is valid Markdown."
                    .to_string(),
            ),
            Self::Internal { detail } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Internal Server Error",
                detail.clone(),
            ),
            Self::NotAcceptable { received } => (
                StatusCode::NOT_ACCEPTABLE,
                "not-acceptable",
                "Not Acceptable",
                received.as_deref().map_or_else(
                    || "Requested output format is not available.".to_string(),
                    |accept| {
                        format!(
                            "Accept '{accept}' is not satisfiable. Use application/vnd.docspec.blocknote+json or */*."
                        )
                    },
                ),
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not-found",
                "Not Found",
                "The requested resource does not exist.".to_string(),
            ),
            Self::UnsupportedMediaType { received } => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported-media-type",
                "Unsupported Media Type",
                received.as_deref().map_or_else(
                    || {
                        "Content-Type header is missing or not supported. Use text/markdown."
                            .to_string()
                    },
                    |ct| format!("Content-Type '{ct}' is not supported. Use text/markdown."),
                ),
            ),
        };

        tracing::debug!(error = %self, "HTTP error response");

        let problem = ProblemJson {
            detail,
            status: status.as_u16(),
            title: title.to_string(),
            type_: format!("https://docspec.dev/errors/{code}"),
        };

        // All values passed to Response::builder() are compile-time constants or derived from
        // StatusCode, so this builder call never fails. The unwrap_or_else fallback is required
        // to satisfy the no-unwrap rule but is unreachable in practice.
        let body = serde_json::to_vec(&problem).unwrap_or_else(
            |_| br#"{"type":"https://docspec.dev/errors/internal","title":"Internal Server Error","status":500,"detail":"Failed to serialize error response."}"#.to_vec(),
        );

        Response::builder()
            .status(status)
            .header(
                axum::http::header::CONTENT_TYPE,
                crate::format::ERROR_PROBLEM_JSON,
            )
            .body(axum::body::Body::from(body))
            .unwrap_or_else(|_| Response::new(axum::body::Body::empty()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    // Reason: Test code uses unwrap and JSON index notation for assertion clarity.

    use axum::body::Body;
    use axum::response::IntoResponse as _;
    use http_body_util::BodyExt as _;

    use super::*;

    async fn body_bytes(body: Body) -> Vec<u8> {
        body.collect().await.unwrap().to_bytes().to_vec()
    }

    fn parse_problem(bytes: &[u8]) -> serde_json::Value {
        serde_json::from_slice(bytes).unwrap()
    }

    #[tokio::test]
    async fn unsupported_media_type_415() {
        let resp = HttpError::UnsupportedMediaType { received: None }.into_response();
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/problem+json"
        );
        let bytes = body_bytes(resp.into_body()).await;
        let json = parse_problem(&bytes);
        assert_eq!(
            json["type"],
            "https://docspec.dev/errors/unsupported-media-type"
        );
        assert_eq!(json["status"], 415);
        assert_eq!(json["title"], "Unsupported Media Type");
    }

    #[tokio::test]
    async fn not_acceptable_406() {
        let resp = HttpError::NotAcceptable {
            received: Some("text/html".into()),
        }
        .into_response();
        assert_eq!(resp.status(), StatusCode::NOT_ACCEPTABLE);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/problem+json"
        );
        let bytes = body_bytes(resp.into_body()).await;
        let json = parse_problem(&bytes);
        assert_eq!(json["type"], "https://docspec.dev/errors/not-acceptable");
        assert_eq!(json["status"], 406);
    }

    #[tokio::test]
    async fn bad_request_400() {
        let resp = HttpError::BadRequest {
            detail: "invalid utf-8".into(),
        }
        .into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = body_bytes(resp.into_body()).await;
        let json = parse_problem(&bytes);
        assert_eq!(json["type"], "https://docspec.dev/errors/bad-request");
        assert_eq!(json["status"], 400);
    }

    #[tokio::test]
    async fn conversion_422() {
        let err = docspec_core::Error::Other {
            message: "parse error".into(),
        };
        let resp = HttpError::Conversion(err).into_response();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let bytes = body_bytes(resp.into_body()).await;
        let json = parse_problem(&bytes);
        assert_eq!(json["type"], "https://docspec.dev/errors/conversion-failed");
        assert_eq!(json["status"], 422);
    }

    #[tokio::test]
    async fn not_found_404() {
        let resp = HttpError::NotFound.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = body_bytes(resp.into_body()).await;
        let json = parse_problem(&bytes);
        assert_eq!(json["type"], "https://docspec.dev/errors/not-found");
        assert_eq!(json["status"], 404);
    }

    #[tokio::test]
    async fn internal_500() {
        let resp = HttpError::Internal {
            detail: "something broke".into(),
        }
        .into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = body_bytes(resp.into_body()).await;
        let json = parse_problem(&bytes);
        assert_eq!(json["type"], "https://docspec.dev/errors/internal");
        assert_eq!(json["status"], 500);
    }

    #[test]
    fn display_messages_cover_all_variants() {
        assert_eq!(
            HttpError::BadRequest {
                detail: "bad utf-8".into(),
            }
            .to_string(),
            "bad request: bad utf-8"
        );
        assert_eq!(
            HttpError::Conversion(docspec_core::Error::Other {
                message: "parse error".into(),
            })
            .to_string(),
            "conversion failed: parse error"
        );
        assert_eq!(
            HttpError::Internal {
                detail: "boom".into(),
            }
            .to_string(),
            "internal error: boom"
        );
        assert_eq!(
            HttpError::NotAcceptable {
                received: Some("text/html".into()),
            }
            .to_string(),
            "not acceptable: text/html"
        );
        assert_eq!(
            HttpError::NotAcceptable { received: None }.to_string(),
            "not acceptable"
        );
        assert_eq!(HttpError::NotFound.to_string(), "not found");
        assert_eq!(
            HttpError::UnsupportedMediaType {
                received: Some("application/json".into()),
            }
            .to_string(),
            "unsupported media type: application/json"
        );
        assert_eq!(
            HttpError::UnsupportedMediaType { received: None }.to_string(),
            "unsupported media type: Content-Type header missing"
        );
    }

    #[test]
    fn source_only_conversion_has_underlying_error() {
        assert!(
            core::error::Error::source(&HttpError::Conversion(docspec_core::Error::Other {
                message: "parse error".into(),
            },))
            .is_some()
        );
        assert!(core::error::Error::source(&HttpError::BadRequest {
            detail: "bad".into(),
        })
        .is_none());
        assert!(core::error::Error::source(&HttpError::Internal {
            detail: "boom".into(),
        })
        .is_none());
        assert!(core::error::Error::source(&HttpError::NotAcceptable { received: None }).is_none());
        assert!(core::error::Error::source(&HttpError::NotFound).is_none());
        assert!(
            core::error::Error::source(&HttpError::UnsupportedMediaType { received: None })
                .is_none()
        );
    }

    #[tokio::test]
    async fn conversion_422_and_not_acceptable_without_received() {
        let conversion = HttpError::Conversion(docspec_core::Error::Other {
            message: "parse error".into(),
        })
        .into_response();
        assert_eq!(conversion.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let conversion_body = body_bytes(conversion.into_body()).await;
        let conversion_json = parse_problem(&conversion_body);
        assert_eq!(conversion_json["status"], 422);

        let not_acceptable = HttpError::NotAcceptable { received: None }.into_response();
        assert_eq!(not_acceptable.status(), StatusCode::NOT_ACCEPTABLE);
        let not_acceptable_body = body_bytes(not_acceptable.into_body()).await;
        let not_acceptable_json = parse_problem(&not_acceptable_body);
        assert_eq!(
            not_acceptable_json["detail"],
            "Requested output format is not available."
        );
    }
}
