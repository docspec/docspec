use std::path::Path;

/// Input format for document conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputFormat {
    /// DOCX (paragraphs and text only). Available when the `docx` feature is enabled.
    #[cfg(feature = "docx")]
    Docx,
    /// HTML (paragraph-only; `<p>` elements and text within them only).
    /// Available when the `html` feature is enabled.
    #[cfg(feature = "html")]
    Html,
    /// Markdown (`CommonMark` + GFM). Available when the `markdown` feature is enabled.
    #[cfg(feature = "markdown")]
    Markdown,
}

/// Output format for document conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    /// `BlockNote` JSON. Available when the `blocknote` feature is enabled.
    #[cfg(feature = "blocknote-writer")]
    Blocknote,
    /// HTML5 (paragraph-only; `<p>` elements and text within them only).
    /// Available when the `html-writer` feature is enabled.
    #[cfg(feature = "html-writer")]
    Html,
    /// `oxa.dev` JSON. Available when the `oxa` feature is enabled.
    #[cfg(feature = "oxa-writer")]
    Oxa,
}

/// Detect the input format from a file path's extension.
///
/// Returns `None` if the extension is unknown or not recognized.
/// Extension matching is case-insensitive.
#[inline]
#[must_use]
pub fn detect_input_format(path: &Path) -> Option<InputFormat> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        #[cfg(feature = "docx")]
        "docx" => Some(InputFormat::Docx),
        #[cfg(feature = "html")]
        "html" | "htm" => Some(InputFormat::Html),
        #[cfg(feature = "markdown")]
        "md" | "markdown" => Some(InputFormat::Markdown),
        _ => None,
    }
}

/// Detect the output format from a file path's extension.
///
/// Returns `None` if the extension is unknown or not recognized.
/// Extension matching is case-insensitive.
///
/// Note: `OutputFormat::Oxa` is not currently auto-detected. Both `BlockNote`
/// and `oxa.dev` emit JSON, so the `.json` extension is ambiguous; callers must
/// select `OutputFormat::Oxa` explicitly.
#[inline]
#[must_use]
pub fn detect_output_format(path: &Path) -> Option<OutputFormat> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        #[cfg(feature = "blocknote-writer")]
        "json" => Some(OutputFormat::Blocknote),
        #[cfg(feature = "html-writer")]
        "html" | "htm" => Some(OutputFormat::Html),
        _ => None,
    }
}

/// Strips a leading UTF-8 BOM from a text input if present. Used by [`crate::AnyReader`]
/// when constructing text-format readers; binary formats never see this helper.
pub(crate) fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{FEFF}').unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_bom_removes_leading_bom() {
        assert_eq!(strip_bom("\u{FEFF}hello"), "hello");
    }

    #[test]
    fn strip_bom_preserves_text_without_bom() {
        assert_eq!(strip_bom("hello"), "hello");
    }

    #[test]
    fn strip_bom_handles_empty_string() {
        assert_eq!(strip_bom(""), "");
    }

    #[test]
    fn strip_bom_preserves_bom_not_at_start() {
        assert_eq!(strip_bom("a\u{FEFF}b"), "a\u{FEFF}b");
    }
}
