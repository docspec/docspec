#![warn(missing_docs)]
//! Command-line interface for `DocSpec` document conversion.

use std::fs::File;
use std::io::IsTerminal as _;

use clap::Parser as _;
use docspec_cli::{
    load_input, resolve_format, run_pipeline, Cli, CliError, ColorChoice, Format, Result,
};

/// Main entry point.
fn main() {
    let cli = Cli::parse();

    let result: Result<()> = (|| {
        if let (Some(input), Some(output)) = (&cli.input, &cli.output) {
            if input == output {
                return Err(CliError::SameInputOutput);
            }
        }

        let input_format = resolve_format(
            cli.from,
            cli.input.as_deref(),
            "cannot detect input format: use --from",
        )?;
        let output_format = resolve_format(
            cli.to,
            cli.output.as_deref(),
            "cannot detect output format: use --to",
        )?;

        let loaded = load_input(cli.input.as_deref())?;
        let content = loaded.as_str();

        if matches!(input_format, Format::Blocknote) {
            return Err(CliError::FormatNotSupported {
                format: "blocknote".to_string(),
            });
        }
        if matches!(output_format, Format::Markdown) {
            return Err(CliError::FormatNotSupported {
                format: "markdown".to_string(),
            });
        }

        cli.output.as_ref().map_or_else(
            || run_pipeline(content, std::io::stdout().lock()),
            |path| run_pipeline(content, File::create(path)?),
        )
    })();

    if let Err(err) = result {
        let use_color = if std::env::var("NO_COLOR").is_ok() {
            false
        } else {
            match cli.color {
                ColorChoice::Always => true,
                ColorChoice::Auto => std::io::stderr().is_terminal(),
                ColorChoice::Never => false,
            }
        };
        let msg = if use_color {
            format!("\x1b[1;31merror:\x1b[0m {err}\n")
        } else {
            format!("error: {err}\n")
        };
        let write_result = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
        drop(write_result);
        std::process::exit(1);
    }
}
