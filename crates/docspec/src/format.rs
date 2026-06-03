use std::path::Path;

/// Input format for document conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputFormat {
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
    #[cfg(feature = "blocknote")]
    Blocknote,
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
        #[cfg(feature = "markdown")]
        "md" | "markdown" => Some(InputFormat::Markdown),
        #[cfg(feature = "html")]
        "html" | "htm" => Some(InputFormat::Html),
        _ => None,
    }
}

/// Detect the output format from a file path's extension.
///
/// Returns `None` if the extension is unknown or not recognized.
/// Extension matching is case-insensitive.
#[inline]
#[must_use]
pub fn detect_output_format(path: &Path) -> Option<OutputFormat> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        #[cfg(feature = "blocknote")]
        "json" => Some(OutputFormat::Blocknote),
        _ => None,
    }
}
