//! MIME type and HTTP header constants for the conversion API.

/// Cache-Control header value applied to all responses.
pub const CACHE_CONTROL_VALUE: &str = "max-age=0, private, must-revalidate";

/// Health endpoint response body.
pub const HEALTH_BODY: &str = "Healthy.";

/// The MIME type accepted as request body.
pub const INPUT_MIME_MARKDOWN: &str = "text/markdown";

/// Accepted alias for the output MIME type (input-only; server always returns primary).
pub const OUTPUT_MIME_ALIAS: &str = "application/vnd.blocknote+json";

/// The primary output MIME type returned on success.
pub const OUTPUT_MIME_PRIMARY: &str = "application/vnd.docspec.blocknote+json";

/// The primary HTML output MIME type returned when the HTML writer is selected.
pub const OUTPUT_MIME_HTML_PRIMARY: &str = "text/html";

/// The primary `oxa.dev` output MIME type returned when the `oxa.dev` writer is selected.
pub const OUTPUT_MIME_OXA_PRIMARY: &str = "application/vnd.oxa+json";

/// MIME type for DOCX documents.
pub const INPUT_MIME_DOCX_PRIMARY: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
