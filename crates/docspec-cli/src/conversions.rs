//! Conversions from CLI enums to facade enums.

use docspec::{InputFormat, OutputFormat};

use crate::args::{CliInputFormat, CliOutputFormat};

impl From<CliInputFormat> for InputFormat {
    #[inline]
    fn from(f: CliInputFormat) -> Self {
        match f {
            CliInputFormat::Markdown => Self::Markdown,
            CliInputFormat::Html => Self::Html,
            CliInputFormat::Docx => Self::Docx,
        }
    }
}

impl From<CliOutputFormat> for OutputFormat {
    #[inline]
    fn from(f: CliOutputFormat) -> Self {
        match f {
            CliOutputFormat::Blocknote => Self::Blocknote,
            CliOutputFormat::Html => Self::Html,
            CliOutputFormat::Oxa => Self::Oxa,
            CliOutputFormat::PandocNative => Self::PandocNative,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_markdown_converts_to_facade_markdown() {
        assert_eq!(
            InputFormat::from(CliInputFormat::Markdown),
            InputFormat::Markdown
        );
    }

    #[test]
    fn from_html_converts_to_facade_html() {
        assert_eq!(InputFormat::from(CliInputFormat::Html), InputFormat::Html);
    }

    #[test]
    fn from_docx_converts_to_facade_docx() {
        assert_eq!(InputFormat::from(CliInputFormat::Docx), InputFormat::Docx);
    }

    #[test]
    fn from_blocknote_converts_to_facade_blocknote() {
        assert_eq!(
            OutputFormat::from(CliOutputFormat::Blocknote),
            OutputFormat::Blocknote
        );
    }

    #[test]
    fn from_html_output_converts_to_facade_html() {
        assert_eq!(
            OutputFormat::from(CliOutputFormat::Html),
            OutputFormat::Html
        );
    }

    #[test]
    fn from_oxa_converts_to_facade_oxa() {
        assert_eq!(OutputFormat::from(CliOutputFormat::Oxa), OutputFormat::Oxa);
    }

    #[test]
    fn from_pandoc_native_converts_to_facade_pandoc_native() {
        assert_eq!(
            OutputFormat::from(CliOutputFormat::PandocNative),
            OutputFormat::PandocNative
        );
    }
}
