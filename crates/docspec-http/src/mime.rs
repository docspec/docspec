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

use docspec::{InputFormat, OutputFormat};

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

/// Input MIME types accepted on `Content-Type`, paired with the reader format
/// each one selects.
///
/// Matching compares the type and subtype only, so parameters and a
/// structured-syntax suffix (`+xml`) are ignored here; callers that need
/// stricter rules (charset, no parameters) apply them on top.
const INPUT_MIMES: &[(&str, InputFormat)] = &[
    (MIME_MARKDOWN, InputFormat::Markdown),
    (MIME_HTML, InputFormat::Html),
    (MIME_DOCX, InputFormat::Docx),
];

/// Output MIME types recognised on `Accept`, paired with the writer format
/// each one selects.
///
/// A format may appear more than once when it has aliases; the first row for
/// a format holds its canonical MIME, which [`output_mime`] returns.
pub(crate) const OUTPUT_MIMES: &[(&str, OutputFormat)] = &[
    (MIME_BLOCKNOTE, OutputFormat::Blocknote),
    (MIME_BLOCKNOTE_ALIAS, OutputFormat::Blocknote),
    (MIME_HTML, OutputFormat::Html),
    (MIME_OXA, OutputFormat::Oxa),
    (MIME_PANDOC_NATIVE, OutputFormat::PandocNative),
];

/// Resolves a parsed `Content-Type` to the reader format whose MIME has the
/// same type and subtype, ignoring parameters and any suffix.
pub(crate) fn classify_input(parsed: &mime::Mime) -> Option<InputFormat> {
    let essence = (parsed.type_().as_str(), parsed.subtype().as_str());
    INPUT_MIMES
        .iter()
        .find(|(input_mime, _)| input_mime.split_once('/') == Some(essence))
        .map(|&(_, format)| format)
}

/// Returns the canonical MIME for a reader format, or `None` if the HTTP API
/// does not accept it.
pub(crate) fn input_mime(format: InputFormat) -> Option<&'static str> {
    INPUT_MIMES
        .iter()
        .find(|&&(_, row_format)| row_format == format)
        .map(|&(input_mime, _)| input_mime)
}

/// Returns the canonical MIME for a writer format, or `None` if the HTTP API
/// does not serve it.
pub(crate) fn output_mime(format: OutputFormat) -> Option<&'static str> {
    OUTPUT_MIMES
        .iter()
        .find(|&&(_, row_format)| row_format == format)
        .map(|&(output_mime, _)| output_mime)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::tests_outside_test_module,
        clippy::unwrap_used,
        clippy::expect_used
    )]

    use super::*;

    #[test]
    fn classify_input_matches_type_and_subtype_ignoring_params() {
        let parsed: mime::Mime = "text/markdown; charset=latin1".parse().unwrap();
        assert_eq!(classify_input(&parsed), Some(InputFormat::Markdown));
    }

    #[test]
    fn classify_input_returns_none_for_unsupported_mime() {
        let parsed: mime::Mime = "application/pdf".parse().unwrap();
        assert_eq!(classify_input(&parsed), None);
    }

    #[test]
    fn input_mime_returns_docx_constant() {
        assert_eq!(input_mime(InputFormat::Docx), Some(MIME_DOCX));
    }

    #[test]
    fn output_mime_returns_canonical_blocknote_not_alias() {
        assert_eq!(output_mime(OutputFormat::Blocknote), Some(MIME_BLOCKNOTE));
    }

    #[test]
    fn output_mime_returns_none_for_format_not_served() {
        assert_eq!(output_mime(OutputFormat::Markdown), None);
    }
}
