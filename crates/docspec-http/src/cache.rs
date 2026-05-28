//! Cache-Control middleware for all HTTP responses.

use axum::http::{header::CACHE_CONTROL, HeaderValue};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::format::CACHE_CONTROL_VALUE;

/// Returns a middleware layer that adds [`CACHE_CONTROL_VALUE`] to every
/// response, overriding any existing Cache-Control header.
///
/// This prevents intermediate proxies and clients from caching conversion results
/// or health check responses.
#[inline]
pub fn cache_control_layer() -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(CACHE_CONTROL, HeaderValue::from_static(CACHE_CONTROL_VALUE))
}
