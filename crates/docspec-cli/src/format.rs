//! Format detection and resolution.

use std::path::Path;

use crate::error::{CliError, Result};
use crate::Format;

/// Resolves format from explicit flag or path detection.
///
/// Uses explicit format if provided, otherwise detects from path extension.
/// Returns an error with the provided message if format cannot be determined.
pub fn resolve_format(
    explicit: Option<Format>,
    path: Option<&Path>,
    error_message: &str,
) -> Result<Format> {
    if let Some(format) = explicit {
        return Ok(format);
    }
    if let Some(p) = path {
        let ext = p.extension().and_then(|e| e.to_str());
        if let Some(format) = ext.and_then(|e| match e.to_ascii_lowercase().as_str() {
            "json" => Some(Format::Blocknote),
            "markdown" | "md" => Some(Format::Markdown),
            _ => None,
        }) {
            return Ok(format);
        }
    }
    Err(CliError::FormatDetection {
        message: error_message.to_string(),
    })
}

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
/// Returns an error with the provided message if format cannot be determined.
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
    Err(CliError::FormatDetection {
        message: error_message.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    // Tests for resolve_format (existing function)
    #[test]
    fn case_insensitive_extension() {
        let result = resolve_format(None, Some(Path::new("doc.MD")), "err");
        assert!(matches!(result, Ok(Format::Markdown)));
    }

    #[test]
    fn detect_blocknote_from_json_extension() {
        let result = resolve_format(None, Some(Path::new("doc.json")), "err");
        assert!(matches!(result, Ok(Format::Blocknote)));
    }

    #[test]
    fn detect_markdown_from_markdown_extension() {
        let result = resolve_format(None, Some(Path::new("doc.markdown")), "err");
        assert!(matches!(result, Ok(Format::Markdown)));
    }

    #[test]
    fn detect_markdown_from_md_extension() {
        let result = resolve_format(None, Some(Path::new("doc.md")), "err");
        assert!(matches!(result, Ok(Format::Markdown)));
    }

    #[test]
    fn explicit_format_overrides_path() {
        let result = resolve_format(Some(Format::Blocknote), Some(Path::new("doc.md")), "err");
        assert!(matches!(result, Ok(Format::Blocknote)));
    }

    #[test]
    fn none_path_returns_error() {
        let result = resolve_format(None, None, "no path err");
        assert!(result.is_err());
        if let Err(CliError::FormatDetection { message }) = result {
            assert_eq!(message, "no path err");
        }
    }

    #[test]
    fn unknown_extension_returns_error() {
        let result = resolve_format(None, Some(Path::new("doc.xyz")), "my error");
        assert!(result.is_err());
        if let Err(CliError::FormatDetection { message }) = result {
            assert_eq!(message, "my error");
        }
    }

    // Tests for resolve_input_format
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
        let result = resolve_input_format(
            Some(CliInputFormat::Html),
            Some(Path::new("doc.md")),
            "err",
        );
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

    // Tests for resolve_output_format
    #[test]
    fn detect_blocknote_from_json_output() {
        let result = resolve_output_format(None, Some(Path::new("doc.json")), "err");
        assert!(matches!(result, Ok(docspec::OutputFormat::Blocknote)));
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
    fn unknown_extension_returns_error_output() {
        let result = resolve_output_format(None, Some(Path::new("doc.xyz")), "my error");
        assert!(result.is_err());
    }

    #[test]
    fn none_path_returns_error_output() {
        let result = resolve_output_format(None, None, "no path err");
        assert!(result.is_err());
    }
}
