//! Centralized MIME type constants for the conversion API.
//!
//! These constants are bare MIME types — no parameters (no `charset=utf-8`,
//! no `q=...`). The `Content-Type` and `Accept` parsers strip parameters
//! before comparing against these values; the conversion handler appends
//! `; charset=utf-8` when constructing response `Content-Type` headers.
//!
//! # Why one canonical home
//!
//! These strings appear in three places at runtime:
//!
//! 1. [`crate::mime_parser`] — validating `Content-Type` and negotiating `Accept`.
//! 2. [`crate::metrics`] — as bounded label values for the `input_mime_type`
//!    and `output_mime_type` Prometheus labels.
//! 3. Integration tests — asserting response shapes and error messages.
//!
//! Defining them once here eliminates the risk that the validator and the
//! metrics layer disagree on what counts as a "supported" MIME.

/// MIME type for `BlockNote` JSON output. The primary output type returned
/// on success when the client did not request a different format.
pub const MIME_BLOCKNOTE: &str = "application/vnd.docspec.blocknote+json";

/// Accepted alias for the `BlockNote` output MIME (input-only on `Accept`;
/// the server always returns [`MIME_BLOCKNOTE`] in its `Content-Type`).
pub const MIME_BLOCKNOTE_ALIAS: &str = "application/vnd.blocknote+json";

/// MIME type for DOCX input. Strict — accepted as `Content-Type` only when
/// sent verbatim with no parameters (binary format; charset is meaningless).
pub const MIME_DOCX: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

/// MIME type for HTML. Accepted as both input `Content-Type` and output
/// `Accept`; emitted as the response `Content-Type` when the HTML writer
/// is selected.
pub const MIME_HTML: &str = "text/html";

/// MIME type for Markdown input. Accepted as `Content-Type` on
/// `POST /conversion`.
pub const MIME_MARKDOWN: &str = "text/markdown";

/// MIME type for `oxa.dev` JSON output, emitted when the `oxa.dev` writer is
/// selected.
pub const MIME_OXA: &str = "application/vnd.oxa+json";

/// MIME type for Pandoc native output, emitted when the Pandoc native writer
/// is selected.
pub const MIME_PANDOC_NATIVE: &str = "application/vnd.pandoc.native";
