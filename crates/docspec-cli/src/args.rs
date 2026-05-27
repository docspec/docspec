use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// `DocSpec`: streaming document conversion.
#[derive(Parser, Debug)]
#[command(name = "docspec", version, about, long_about = None)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub struct Cli {
    /// When to use colors.
    #[arg(long, value_name = "WHEN", default_value = "auto", global = true)]
    pub color: ColorChoice,

    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Available subcommands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Convert documents between formats.
    Convert(ConvertArgs),
    /// Start the HTTP API server.
    Http(HttpArgs),
}

/// Arguments for the `convert` subcommand.
#[derive(Args, Debug)]
pub struct ConvertArgs {
    /// Input format (auto-detected from extension if omitted).
    #[arg(short, long)]
    pub from: Option<Format>,

    /// Input file (use `-` or omit for stdin).
    #[arg(value_name = "INPUT")]
    pub input: Option<PathBuf>,

    /// List supported input formats and exit.
    #[arg(long)]
    pub list_input_formats: bool,

    /// List supported output formats and exit.
    #[arg(long)]
    pub list_output_formats: bool,

    /// Output file (stdout if omitted).
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Output format (auto-detected from extension if omitted).
    #[arg(short, long)]
    pub to: Option<Format>,

    /// Print a "conversion complete" message to stderr when done.
    #[arg(long)]
    pub verbose: bool,
}

/// Arguments for the `http` subcommand.
#[derive(Args, Debug)]
pub struct HttpArgs {
    /// Host address to bind to.
    #[arg(long, default_value = "127.0.0.1", env = "DOCSPEC_HTTP_HOST")]
    pub host: String,

    /// Log format.
    #[arg(long, default_value = "pretty", env = "DOCSPEC_LOG_FORMAT")]
    pub log_format: LogFormatArg,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, default_value = "info", env = "DOCSPEC_LOG_LEVEL")]
    pub log_level: String,

    /// Port to listen on.
    #[arg(long, default_value = "3000", env = "DOCSPEC_HTTP_PORT")]
    pub port: u16,
}

/// Log format selection for the HTTP server.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LogFormatArg {
    /// Machine-readable JSON logs.
    #[value(name = "json")]
    Json,
    /// Human-readable pretty-printed logs.
    #[value(name = "pretty")]
    Pretty,
}

/// Document format.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Format {
    /// `BlockNote` JSON format.
    #[value(name = "blocknote")]
    Blocknote,

    /// Markdown format.
    #[value(name = "markdown")]
    Markdown,
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
