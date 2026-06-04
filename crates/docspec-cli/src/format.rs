//! Format detection and resolution.

use std::path::Path;

use crate::error::{CliError, Result};

/// Resolves input format from explicit flag or path detection.
///
/// Uses explicit format if provided, otherwise detects from path extension via the facade.
/// Returns an error with the provided message if format cannot be determined.
pub fn resolve_input_format(
    explicit: Option<crate::args::CliInputFormat>,
    path: Option<&Path>,
    error_message: &str,
) -> Result<docspec::InputFormat> {
    if let Some(format) = explicit {
        return Ok(format.into());
    }
    if let Some(p) = path {
        if let Some(format) = docspec::detect_input_format(p) {
            return Ok(format);
        }
    }
    Err(CliError::FormatDetection {
        message: error_message.to_string(),
    })
}

/// Resolves output format from explicit flag or path detection.
///
/// Uses explicit format if provided, otherwise detects from path extension via the facade.
/// Defaults to `BlockNote` when no output format can be detected: the `.json` extension is
/// ambiguous between writers, so `oxa.dev` must be selected explicitly via `--to oxa`.
pub fn resolve_output_format(
    explicit: Option<crate::args::CliOutputFormat>,
    path: Option<&Path>,
    error_message: &str,
) -> Result<docspec::OutputFormat> {
    if let Some(format) = explicit {
        return Ok(format.into());
    }
    if let Some(p) = path {
        if let Some(format) = docspec::detect_output_format(p) {
            return Ok(format);
        }
    }
    let _ = error_message;
    Ok(docspec::OutputFormat::Blocknote)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn detect_html_from_html() {
        let result = resolve_input_format(None, Some(Path::new("doc.html")), "err");
        assert!(matches!(result, Ok(docspec::InputFormat::Html)));
    }

    #[test]
    fn detect_html_from_htm() {
        let result = resolve_input_format(None, Some(Path::new("doc.htm")), "err");
        assert!(matches!(result, Ok(docspec::InputFormat::Html)));
    }

    #[test]
    fn case_insensitive_extension_input() {
        let result = resolve_input_format(None, Some(Path::new("doc.HTML")), "err");
        assert!(matches!(result, Ok(docspec::InputFormat::Html)));
    }

    #[test]
    fn detect_markdown_from_md_input() {
        let result = resolve_input_format(None, Some(Path::new("doc.md")), "err");
        assert!(matches!(result, Ok(docspec::InputFormat::Markdown)));
    }

    #[test]
    fn explicit_format_overrides_path_input() {
        use crate::args::CliInputFormat;
        let result =
            resolve_input_format(Some(CliInputFormat::Html), Some(Path::new("doc.md")), "err");
        assert!(matches!(result, Ok(docspec::InputFormat::Html)));
    }

    #[test]
    fn unknown_extension_returns_error_input() {
        let result = resolve_input_format(None, Some(Path::new("doc.xyz")), "my error");
        assert!(
            matches!(&result, Err(CliError::FormatDetection { message }) if message == "my error"),
            "expected FormatDetection {{ message: \"my error\" }}, got {result:?}"
        );
    }

    #[test]
    fn none_path_returns_error_input() {
        let result = resolve_input_format(None, None, "no path err");
        assert!(
            matches!(&result, Err(CliError::FormatDetection { message }) if message == "no path err"),
            "expected FormatDetection {{ message: \"no path err\" }}, got {result:?}"
        );
    }

    #[test]
    fn detect_blocknote_from_json_output() {
        let result = resolve_output_format(None, Some(Path::new("doc.json")), "err");
        assert!(matches!(result, Ok(docspec::OutputFormat::Blocknote)));
    }

    #[test]
    fn detect_html_from_html_output() {
        let result = resolve_output_format(None, Some(Path::new("doc.html")), "err");
        assert!(matches!(result, Ok(docspec::OutputFormat::Html)));
    }

    #[test]
    fn detect_html_from_htm_output() {
        let result = resolve_output_format(None, Some(Path::new("doc.htm")), "err");
        assert!(matches!(result, Ok(docspec::OutputFormat::Html)));
    }

    #[test]
    fn case_insensitive_extension_output() {
        let result = resolve_output_format(None, Some(Path::new("doc.JSON")), "err");
        assert!(matches!(result, Ok(docspec::OutputFormat::Blocknote)));
    }

    #[test]
    fn explicit_format_overrides_path_output() {
        use crate::args::CliOutputFormat;
        let result = resolve_output_format(
            Some(CliOutputFormat::Blocknote),
            Some(Path::new("doc.md")),
            "err",
        );
        assert!(matches!(result, Ok(docspec::OutputFormat::Blocknote)));
    }

    #[test]
    fn unknown_extension_defaults_to_blocknote_output() {
        let result = resolve_output_format(None, Some(Path::new("doc.xyz")), "my error");
        assert!(matches!(result, Ok(docspec::OutputFormat::Blocknote)));
    }

    #[test]
    fn none_path_defaults_to_blocknote_output() {
        let result = resolve_output_format(None, None, "no path err");
        assert!(matches!(result, Ok(docspec::OutputFormat::Blocknote)));
    }

    #[test]
    fn json_path_without_explicit_flag_defaults_to_blocknote_not_oxa() {
        // Regression guard: .json is ambiguous; auto-detection must NOT pick oxa.
        let result = resolve_output_format(None, Some(Path::new("doc.json")), "err");
        assert!(matches!(result, Ok(docspec::OutputFormat::Blocknote)));
    }
}
