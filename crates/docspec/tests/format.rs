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

    #[cfg(feature = "blocknote-writer")]
    #[test]
    fn detect_blocknote_from_json() {
        let result = docspec::detect_output_format(Path::new("file.json"));
        assert_eq!(result, Some(docspec::OutputFormat::Blocknote));
    }

    #[cfg(feature = "blocknote-writer")]
    #[test]
    fn case_insensitive_json() {
        let result = docspec::detect_output_format(Path::new("file.JSON"));
        assert_eq!(result, Some(docspec::OutputFormat::Blocknote));
    }

    #[cfg(all(feature = "oxa-writer", not(feature = "blocknote-writer")))]
    #[test]
    fn oxa_is_not_auto_detected_from_json() {
        let result = docspec::detect_output_format(Path::new("file.json"));
        assert_eq!(result, None);
    }

    #[cfg(feature = "html-writer")]
    #[test]
    fn detect_html_output_from_html() {
        let result = docspec::detect_output_format(Path::new("file.html"));
        assert_eq!(result, Some(docspec::OutputFormat::Html));
    }

    #[cfg(feature = "html-writer")]
    #[test]
    fn detect_html_output_from_htm() {
        let result = docspec::detect_output_format(Path::new("file.htm"));
        assert_eq!(result, Some(docspec::OutputFormat::Html));
    }

    #[cfg(feature = "html-writer")]
    #[test]
    fn case_insensitive_html_output() {
        let result = docspec::detect_output_format(Path::new("file.HTML"));
        assert_eq!(result, Some(docspec::OutputFormat::Html));
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

    #[cfg(feature = "blocknote-writer")]
    #[test]
    fn output_format_debug() {
        let debug_str = format!("{:?}", docspec::OutputFormat::Blocknote);
        assert_eq!(debug_str, "Blocknote");
    }

    #[cfg(feature = "oxa-writer")]
    #[test]
    fn output_format_debug_oxa() {
        let debug_str = format!("{:?}", docspec::OutputFormat::Oxa);
        assert_eq!(debug_str, "Oxa");
    }

    #[cfg(feature = "oxa-writer")]
    #[test]
    fn output_format_eq_oxa() {
        assert_eq!(docspec::OutputFormat::Oxa, docspec::OutputFormat::Oxa);
    }

    #[cfg(all(feature = "blocknote-writer", feature = "oxa-writer"))]
    #[test]
    fn output_format_ne_oxa_blocknote() {
        assert_ne!(docspec::OutputFormat::Oxa, docspec::OutputFormat::Blocknote);
    }

    #[cfg(feature = "html-writer")]
    #[test]
    fn output_format_debug_html() {
        let debug_str = format!("{:?}", docspec::OutputFormat::Html);
        assert_eq!(debug_str, "Html");
    }

    #[cfg(feature = "html-writer")]
    #[test]
    fn output_format_eq_html() {
        assert_eq!(docspec::OutputFormat::Html, docspec::OutputFormat::Html);
    }

    #[cfg(all(feature = "blocknote-writer", feature = "html-writer"))]
    #[test]
    fn output_format_ne_html_blocknote() {
        assert_ne!(
            docspec::OutputFormat::Html,
            docspec::OutputFormat::Blocknote
        );
    }

    #[cfg(all(feature = "oxa-writer", feature = "html-writer"))]
    #[test]
    fn output_format_ne_html_oxa() {
        assert_ne!(docspec::OutputFormat::Html, docspec::OutputFormat::Oxa);
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn input_format_eq() {
        assert_eq!(
            docspec::InputFormat::Markdown,
            docspec::InputFormat::Markdown
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn detect_docx_from_docx_extension() {
        let result = docspec::detect_input_format(Path::new("file.docx"));
        assert_eq!(result, Some(docspec::InputFormat::Docx));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn detect_docx_case_insensitive() {
        let result_upper = docspec::detect_input_format(Path::new("file.DOCX"));
        assert_eq!(result_upper, Some(docspec::InputFormat::Docx));
        let result_mixed = docspec::detect_input_format(Path::new("file.Docx"));
        assert_eq!(result_mixed, Some(docspec::InputFormat::Docx));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn docx_input_format_debug() {
        let debug_str = format!("{:?}", docspec::InputFormat::Docx);
        assert_eq!(debug_str, "Docx");
    }

    #[cfg(feature = "docx")]
    #[test]
    fn docx_input_format_eq() {
        assert_eq!(docspec::InputFormat::Docx, docspec::InputFormat::Docx);
    }

    #[cfg(all(feature = "docx", feature = "markdown"))]
    #[test]
    fn input_format_ne_docx_markdown() {
        assert_ne!(docspec::InputFormat::Docx, docspec::InputFormat::Markdown);
    }
}
