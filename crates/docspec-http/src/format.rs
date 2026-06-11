//! HTTP header value and body constants for the conversion API.
//!
//! MIME type constants live in the sibling [`crate::mime`] module.

/// Cache-Control header value applied to all responses.
pub const CACHE_CONTROL_VALUE: &str = "max-age=0, private, must-revalidate";

/// Health endpoint response body.
pub const HEALTH_BODY: &str = "Healthy.";
