use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Process exit statuses as `(code, sysexits name, meaning)`.
///
/// The one source for the `--help` footer, the man page's EXIT STATUS section
/// (`build.rs`) and the README's "Exit status" table, which a test checks.
pub const EXIT_STATUSES: &[(u8, &str, &str)] = &[
    (0, "EX_OK", "Success"),
    (2, "usage", "Command used incorrectly: unknown flag or value, missing subcommand, input format not given and not detectable, input and output are the same file"),
    (65, "EX_DATAERR", "The input document is malformed or holds something that cannot be converted"),
    (66, "EX_NOINPUT", "The input file does not exist or is a directory"),
    (69, "EX_UNAVAILABLE", "The HTTP server cannot listen: the address is already in use or not available"),
    (70, "EX_SOFTWARE", "Internal DocSpec error, such as a reader violating the event contract"),
    (71, "EX_OSERR", "The operating system refused a resource: runtime or socket creation failed"),
    (73, "EX_CANTCREAT", "The output file cannot be created"),
    (74, "EX_IOERR", "I/O failed while reading the input or writing the output"),
    (77, "EX_NOPERM", "Permission denied on the input, the output, or the listen port"),
];

/// The `--help` footer, rendered from [`EXIT_STATUSES`].
fn exit_status_help() -> String {
    let rows = EXIT_STATUSES
        .iter()
        .map(|(code, _, meaning)| format!("  {code:<3} {meaning}\n"))
        .collect::<Vec<_>>()
        .concat();
    format!("Exit status:\n{rows}\nCodes above 2 follow <sysexits.h>.")
}

/// `DocSpec`: streaming document conversion.
#[derive(Parser, Debug)]
#[command(
    name = "docspec",
    version,
    about = "Streaming document conversion CLI",
    subcommand_required = true,
    arg_required_else_help = true,
    after_long_help = exit_status_help()
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
    #[command(
        long_about = "Convert documents between formats using a streaming event pipeline.\n\nInput: Markdown and DOCX are supported. DOCX preserves headings, tables, lists, hyperlinks, images, run styles, and colors. Known DOCX losses include vertical cell merges, comments, footnotes, headers and footers, document metadata, tracked deletions, and field-code hyperlinks. HTML input is experimental and preserves only <p> elements and their text; all other tags are dropped. ODT input is a placeholder: the file is ignored and the output is an empty document.\n\nOutput: BlockNote JSON is supported. HTML, oxa.dev JSON, Pandoc native, and Markdown outputs are experimental and silently drop structure they cannot express.",
        after_long_help = exit_status_help()
    )]
    Convert(ConvertArgs),
    /// Run the HTTP API server.
    #[cfg(feature = "http")]
    Http(HttpArgs),
}

/// Arguments for the `convert` subcommand.
#[derive(clap::Args, Debug)]
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
    /// ODT — PLACEHOLDER: the input is ignored and the output is an empty document.
    #[value(name = "odt")]
    Odt,
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

    /// The README's "Exit status" table is a hand-written copy of
    /// `EXIT_STATUSES`; this keeps it from drifting.
    #[test]
    fn readme_exit_status_table_matches_exit_statuses() {
        let readme = include_str!("../README.md");
        let rows: Vec<(u8, &str, &str)> = readme
            .lines()
            .filter_map(|line| {
                let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                let &[_, code, name, meaning, _] = cells.as_slice() else {
                    return None;
                };
                Some((code.parse().ok()?, name.trim_matches('`'), meaning))
            })
            .collect();
        assert_eq!(rows, EXIT_STATUSES);
    }

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
