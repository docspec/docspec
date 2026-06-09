use std::path::Path;

/// Input format for document conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InputFormat {
    /// DOCX format (paragraphs and text only). Available when the `docx` feature is enabled.
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
#[non_exhaustive]
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
    /// Pandoc native block-list syntax. Available when the `pandoc-native` feature is enabled.
    #[cfg(feature = "pandoc-native-writer")]
    PandocNative,
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
        #[cfg(feature = "html-writer")]
        "html" | "htm" => Some(OutputFormat::Html),
        #[cfg(feature = "blocknote-writer")]
        "json" => Some(OutputFormat::Blocknote),
        #[cfg(feature = "pandoc-native-writer")]
        "native" => Some(OutputFormat::PandocNative),
        _ => None,
    }
}

/// Strip a UTF-8 byte-order mark (U+FEFF) from the start of `input`, if present.
///
/// Returns a subslice of `input` with the leading BOM removed, or `input`
/// unchanged if no BOM is present.
#[inline]
#[must_use]
pub fn strip_bom(input: &str) -> &str {
    input.strip_prefix('\u{FEFF}').unwrap_or(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_bom_empty_string() {
        assert_eq!(strip_bom(""), "");
    }

    #[test]
    fn strip_bom_no_bom() {
        assert_eq!(strip_bom("hello"), "hello");
    }

    #[test]
    fn strip_bom_with_bom() {
        assert_eq!(strip_bom("\u{FEFF}hello"), "hello");
    }

    #[test]
    fn strip_bom_bom_only() {
        assert_eq!(strip_bom("\u{FEFF}"), "");
    }

    #[cfg(feature = "docx")]
    #[test]
    fn detect_input_format_recognizes_docx() {
        use std::path::Path;
        assert_eq!(
            detect_input_format(Path::new("a.docx")),
            Some(InputFormat::Docx)
        );
        assert_eq!(
            detect_input_format(Path::new("a.DOCX")),
            Some(InputFormat::Docx)
        );
        assert_eq!(
            detect_input_format(Path::new("a.DocX")),
            Some(InputFormat::Docx)
        );
    }
}
