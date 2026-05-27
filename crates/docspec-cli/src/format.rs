//! Format detection and resolution.

use std::path::Path;

use crate::error::{CliError, Result};
use crate::Format;

/// Resolves format from explicit flag or path detection.
///
/// Uses explicit format if provided, otherwise detects from path extension.
/// Returns an error with the provided message if format cannot be determined.
///
/// # Errors
///
/// Returns an error if no explicit format is provided and the path extension is
/// missing or unrecognized.
#[inline]
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

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
}
