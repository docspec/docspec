#![warn(missing_docs)]
//! Command-line interface for `DocSpec` document conversion.

mod args;
mod error;
mod format;

use std::fs::File;
use std::io::{IsTerminal as _, Read as _, Write};

use clap::Parser as _;
use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::{EventSink as _, EventSource as _, StackTrackingSink};
use docspec_markdown_reader::MarkdownReader;

use crate::args::{Cli, ColorChoice, Format};
use crate::error::{CliError, Result};

/// Runs the streaming conversion pipeline from markdown to `BlockNote`.
fn run_pipeline<W: Write>(content: &str, output: W) -> Result<()> {
    let mut reader = MarkdownReader::new(content);
    let mut sink = StackTrackingSink::new(BlockNoteWriter::new(output));

    let mut next = reader.next_event();
    while let Ok(Some(event)) = next {
        sink.handle_event(event)?;
        next = reader.next_event();
    }
    if let Err(e) = next {
        return Err(e.into());
    }
    sink.finish()?;
    Ok(())
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

        let input_format = format::resolve_format(
            cli.from,
            cli.input.as_deref(),
            "cannot detect input format: use --from",
        )?;
        let output_format = format::resolve_format(
            cli.to,
            cli.output.as_deref(),
            "cannot detect output format: use --to",
        )?;

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
            || run_pipeline(&content, std::io::stdout().lock()),
            |path| run_pipeline(&content, File::create(path)?),
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
