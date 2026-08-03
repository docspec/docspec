use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// `DocSpec`: streaming document conversion.
#[derive(Parser, Debug)]
#[command(
    name = "docspec",
    version,
    about = "Streaming document conversion CLI",
    subcommand_required = true,
    arg_required_else_help = true
)]
pub struct Cli {
    /// Selected top-level subcommand.
    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level subcommands for `docspec`.
#[derive(Subcommand, Debug)]
#[non_exhaustive]
pub enum Commands {
    /// Convert documents between formats.
    Convert(ConvertArgs),
    /// Run the HTTP API server.
    #[cfg(feature = "http")]
    Http(HttpArgs),
}

/// Arguments for the `convert` subcommand.
#[derive(clap::Args, Debug)]
#[command(
    about = "Convert documents between formats using streaming event pipeline",
    long_about = "Convert documents between formats using streaming event pipeline.\n\nSupports converting Markdown, HTML, or DOCX input to BlockNote JSON, HTML, oxa.dev JSON, or Pandoc native output.\n\nNote: HTML and DOCX input currently preserve only paragraph text. Other HTML input\nelements and non-paragraph output events (headings, lists, tables, formatting, etc.)\nare silently dropped. DOCX input preserves only paragraphs and text; styles, tables,\nlists, images, headers/footers, and tracked changes are silently dropped. Use BlockNote\nJSON output for fuller feature coverage."
)]
pub struct ConvertArgs {
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

/// Arguments for the `http` subcommand.
#[cfg(feature = "http")]
#[derive(clap::Args, Debug, Clone)]
#[non_exhaustive]
pub struct HttpArgs {
    /// Host to bind to.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Port to bind to.
    #[arg(long, default_value_t = 3000)]
    pub port: u16,
}

/// Color output choice.
#[derive(Clone, Copy, Debug, ValueEnum)]
#[non_exhaustive]
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
#[non_exhaustive]
pub enum CliInputFormat {
    /// DOCX — supported: headings, tables, lists, hyperlinks, images, run styles.
    #[value(name = "docx")]
    Docx,
    /// HTML — EXPERIMENTAL: `<p>` and its text only; all other tags are dropped.
    #[value(name = "html")]
    Html,
    /// Markdown — supported: `CommonMark` plus GFM tables and strikethrough.
    #[value(name = "markdown")]
    Markdown,
}

/// Output format for document conversion.
#[derive(Clone, Copy, Debug, ValueEnum)]
#[non_exhaustive]
pub enum CliOutputFormat {
    /// `BlockNote` JSON — supported: headings, lists, tables, links, images, styles.
    #[value(name = "blocknote")]
    Blocknote,
    /// HTML5 — EXPERIMENTAL: paragraphs and text only; headings, lists, tables,
    /// links and images are silently dropped.
    #[value(name = "html")]
    Html,
    /// `oxa.dev` JSON — EXPERIMENTAL: paragraphs and text only; headings, lists,
    /// tables, links and images are silently dropped.
    #[value(name = "oxa")]
    Oxa,
    /// Pandoc native — EXPERIMENTAL: paragraphs, headings, styles and code only;
    /// no lists, tables, links or images.
    #[value(name = "pandoc-native")]
    PandocNative,
    /// Markdown — EXPERIMENTAL: paragraphs and headings only, text only.
    #[value(name = "markdown")]
    Markdown,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::infallible_destructuring_match)]
    use super::*;

    #[test]
    fn clap_rejects_blocknote_as_input() {
        let result = Cli::try_parse_from(["docspec", "convert", "--from", "blocknote", "x.md"]);
        assert!(
            result.is_err(),
            "blocknote should not be a valid input format"
        );
    }

    #[test]
    fn clap_accepts_markdown_as_output() {
        let result = Cli::try_parse_from(["docspec", "convert", "--to", "markdown", "x.md"]);
        assert!(
            result.is_ok(),
            "markdown should be a valid output format, got error: {:?}",
            result.as_ref().err()
        );
        let cli = result.unwrap_or_else(|_| std::process::abort());
        let args = match cli.command {
            Commands::Convert(args) => args,
            #[cfg(feature = "http")]
            Commands::Http(_) => std::process::abort(),
        };
        assert!(
            matches!(args.to, Some(CliOutputFormat::Markdown)),
            "expected CliOutputFormat::Markdown, got {:?}",
            args.to
        );
    }

    #[test]
    fn clap_accepts_html_as_output_format() {
        let result = Cli::try_parse_from(["docspec", "convert", "--to", "html", "x.md"]);
        assert!(
            result.is_ok(),
            "html should be a valid output format, got error: {:?}",
            result.as_ref().err()
        );
        let cli = result.unwrap_or_else(|_| std::process::abort());
        let args = match cli.command {
            Commands::Convert(args) => args,
            #[cfg(feature = "http")]
            Commands::Http(_) => std::process::abort(),
        };
        assert!(
            matches!(args.to, Some(CliOutputFormat::Html)),
            "expected CliOutputFormat::Html, got {:?}",
            args.to
        );
    }

    #[test]
    fn clap_accepts_oxa_as_output_format() {
        let result = Cli::try_parse_from(["docspec", "convert", "--to", "oxa", "x.md"]);
        assert!(
            result.is_ok(),
            "oxa should be a valid output format, got error: {:?}",
            result.as_ref().err()
        );
        let cli = result.unwrap_or_else(|_| std::process::abort());
        let args = match cli.command {
            Commands::Convert(args) => args,
            #[cfg(feature = "http")]
            Commands::Http(_) => std::process::abort(),
        };
        assert!(
            matches!(args.to, Some(CliOutputFormat::Oxa)),
            "expected CliOutputFormat::Oxa, got {:?}",
            args.to
        );
    }

    #[test]
    fn clap_accepts_pandoc_native_as_output_format() {
        let result = Cli::try_parse_from(["docspec", "convert", "--to", "pandoc-native", "x.md"]);
        assert!(
            result.is_ok(),
            "pandoc-native should be a valid output format, got error: {:?}",
            result.as_ref().err()
        );
        let cli = result.unwrap_or_else(|_| std::process::abort());
        let args = match cli.command {
            Commands::Convert(args) => args,
            #[cfg(feature = "http")]
            Commands::Http(_) => std::process::abort(),
        };
        assert!(
            matches!(args.to, Some(CliOutputFormat::PandocNative)),
            "expected CliOutputFormat::PandocNative, got {:?}",
            args.to
        );
    }
}
