#![warn(missing_docs)]
//! Command-line interface for `DocSpec` document conversion.

mod args;
mod conversions;
mod error;
mod format;

use std::fs::File;
use std::io::{BufWriter, IsTerminal as _, Read as _, Write};

use clap::Parser as _;
use docspec::{AnyReader, AnyWriter};

use crate::args::{Cli, ColorChoice};
use crate::error::{CliError, Result};

/// Runs the streaming conversion pipeline.
fn run_pipeline<W: Write>(
    input_format: docspec::InputFormat,
    content: &str,
    output_format: docspec::OutputFormat,
    output: W,
) -> Result<()> {
    let reader = AnyReader::new(input_format, content);
    let sink = AnyWriter::new(output_format, output);
    docspec_core::pipe(reader, sink).map_err(Into::into)
}

/// Main entry point.
fn main() {
    let cli = Cli::parse();

    let result: Result<()> = (|| {
        if let (Some(input), Some(output)) = (&cli.input, &cli.output) {
            if input == output {
                return Err(CliError::SameInputOutput);
            }
        }

        // Resolve formats BEFORE reading input (fail-fast)
        let input_format = format::resolve_input_format(
            cli.from,
            cli.input.as_deref(),
            "cannot detect input format: use --from",
        )?;
        let output_format = format::resolve_output_format(
            cli.to,
            cli.output.as_deref(),
            "cannot detect output format: use --to",
        )?;

        // Read input AFTER format validation
        let raw_content = match cli.input.as_ref() {
            None => {
                let mut buf = String::new();
                std::io::stdin().lock().read_to_string(&mut buf)?;
                buf
            }
            Some(path) if path.as_os_str() == "-" => {
                let mut buf = String::new();
                std::io::stdin().lock().read_to_string(&mut buf)?;
                buf
            }
            Some(path) => std::fs::read_to_string(path)?,
        };
        let content = raw_content
            .strip_prefix('\u{FEFF}')
            .unwrap_or(&raw_content)
            .to_string();

        cli.output.as_ref().map_or_else(
            || run_pipeline(input_format, &content, output_format, std::io::stdout().lock()),
            |path| {
                let mut writer = BufWriter::new(File::create(path)?);
                run_pipeline(input_format, &content, output_format, &mut writer)?;
                writer.flush()?;
                Ok(())
            },
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
