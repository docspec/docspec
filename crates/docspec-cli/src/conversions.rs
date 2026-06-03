//! Conversions from CLI enums to facade enums.

use docspec::{InputFormat, OutputFormat};

use crate::args::{CliInputFormat, CliOutputFormat};

impl From<CliInputFormat> for InputFormat {
    #[inline]
    fn from(f: CliInputFormat) -> Self {
        match f {
            CliInputFormat::Markdown => Self::Markdown,
            CliInputFormat::Html => Self::Html,
        }
    }
}

impl From<CliOutputFormat> for OutputFormat {
    #[inline]
    fn from(f: CliOutputFormat) -> Self {
        match f {
            CliOutputFormat::Blocknote => Self::Blocknote,
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
    fn from_blocknote_converts_to_facade_blocknote() {
        assert_eq!(
            OutputFormat::from(CliOutputFormat::Blocknote),
            OutputFormat::Blocknote
        );
    }
}
