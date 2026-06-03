use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// `DocSpec`: streaming document conversion.
#[derive(Parser, Debug)]
#[command(name = "docspec")]
#[command(version = "0.1.0")]
#[command(about = "Convert documents between formats using streaming event pipeline", long_about = None)]
pub struct Cli {
    /// When to use colors.
    #[arg(long, value_name = "WHEN", default_value = "auto")]
    pub color: ColorChoice,

    /// Input format (auto-detected from extension if omitted).
    #[arg(short, long)]
    pub from: Option<CliInputFormat>,

    /// Input file (use `-` or omit for stdin).
    #[arg(value_name = "FILE")]
    pub input: Option<PathBuf>,

    /// Output file (stdout if omitted).
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Output format (auto-detected from extension if omitted).
    #[arg(short, long)]
    pub to: Option<CliOutputFormat>,
}

/// Color output choice.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorChoice {
    /// Always use colors.
    #[value(name = "always")]
    Always,

    /// Automatically detect color support.
    #[value(name = "auto")]
    Auto,

    /// Never use colors.
    #[value(name = "never")]
    Never,
}

/// Input format for document conversion.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliInputFormat {
    /// HTML format (paragraph-only; `<p>` elements and text within them only).
    #[value(name = "html")]
    Html,
    /// Markdown format.
    #[value(name = "markdown")]
    Markdown,
}

/// Output format for document conversion.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliOutputFormat {
    /// `BlockNote` JSON format.
    #[value(name = "blocknote")]
    Blocknote,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_rejects_blocknote_as_input() {
        let result = Cli::try_parse_from(["docspec", "--from", "blocknote", "x.md"]);
        assert!(result.is_err(), "blocknote should not be a valid input format");
    }

    #[test]
    fn clap_rejects_markdown_as_output() {
        let result = Cli::try_parse_from(["docspec", "--to", "markdown", "x.md"]);
        assert!(result.is_err(), "markdown should not be a valid output format");
    }
}
