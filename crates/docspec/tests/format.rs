//! Integration tests for the format module.

use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "markdown")]
    #[test]
    fn detect_markdown_from_md() {
        let result = docspec::detect_input_format(Path::new("file.md"));
        assert_eq!(result, Some(docspec::InputFormat::Markdown));
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn detect_markdown_from_markdown() {
        let result = docspec::detect_input_format(Path::new("file.markdown"));
        assert_eq!(result, Some(docspec::InputFormat::Markdown));
    }

    #[cfg(feature = "html")]
    #[test]
    fn detect_html_from_html() {
        let result = docspec::detect_input_format(Path::new("file.html"));
        assert_eq!(result, Some(docspec::InputFormat::Html));
    }

    #[cfg(feature = "html")]
    #[test]
    fn detect_html_from_htm() {
        let result = docspec::detect_input_format(Path::new("file.htm"));
        assert_eq!(result, Some(docspec::InputFormat::Html));
    }

    #[cfg(feature = "html")]
    #[test]
    fn case_insensitive_html() {
        let result = docspec::detect_input_format(Path::new("file.HTML"));
        assert_eq!(result, Some(docspec::InputFormat::Html));
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn case_insensitive_md() {
        let result = docspec::detect_input_format(Path::new("file.MD"));
        assert_eq!(result, Some(docspec::InputFormat::Markdown));
    }

    #[cfg(feature = "blocknote")]
    #[test]
    fn detect_blocknote_from_json() {
        let result = docspec::detect_output_format(Path::new("file.json"));
        assert_eq!(result, Some(docspec::OutputFormat::Blocknote));
    }

    #[cfg(feature = "blocknote")]
    #[test]
    fn case_insensitive_json() {
        let result = docspec::detect_output_format(Path::new("file.JSON"));
        assert_eq!(result, Some(docspec::OutputFormat::Blocknote));
    }

    #[test]
    fn unknown_extension_returns_none() {
        let result = docspec::detect_input_format(Path::new("file.txt"));
        assert_eq!(result, None);
    }

    #[test]
    fn no_extension_returns_none() {
        let result = docspec::detect_input_format(Path::new("file"));
        assert_eq!(result, None);
    }

    #[test]
    fn unknown_output_extension_returns_none() {
        let result = docspec::detect_output_format(Path::new("file.txt"));
        assert_eq!(result, None);
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn input_format_debug() {
        let debug_str = format!("{:?}", docspec::InputFormat::Markdown);
        assert_eq!(debug_str, "Markdown");
    }

    #[cfg(feature = "blocknote")]
    #[test]
    fn output_format_debug() {
        let debug_str = format!("{:?}", docspec::OutputFormat::Blocknote);
        assert_eq!(debug_str, "Blocknote");
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn input_format_eq() {
        assert_eq!(docspec::InputFormat::Markdown, docspec::InputFormat::Markdown);
    }
}
