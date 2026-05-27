#![warn(missing_docs)]
//! Command-line interface for `DocSpec` document conversion.

mod args;
mod error;
mod format;

use std::fs::File;
use std::io::{BufWriter, IsTerminal as _, Read as _, Write};
use std::path::Path;

use clap::Parser as _;
use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::StackTrackingSink;
use docspec_markdown_reader::MarkdownReader;

use crate::args::{Cli, ColorChoice, Command, ConvertArgs, Format, HttpArgs, LogFormatArg};
use crate::error::{CliError, Result};

impl From<LogFormatArg> for docspec_http::tracing_init::LogFormat {
    #[inline]
    fn from(arg: LogFormatArg) -> Self {
        match arg {
            LogFormatArg::Json => Self::Json,
            LogFormatArg::Pretty => Self::Pretty,
        }
    }
}

// Reason: These functions are called exactly once from main() — that is intentional
// for the subcommand dispatch pattern. They will gain more callers in tests (T13, T15).
#[allow(clippy::single_call_fn)]
// Reason: run_convert has multiple sequential validation stages by design; extracting
// them into separate helpers would add indirection without improving clarity.
#[allow(clippy::cognitive_complexity)]
fn run_convert(args: &ConvertArgs, _color: ColorChoice) -> Result<()> {
    if args.list_input_formats {
        let mut out = std::io::stdout().lock();
        for name in format::input_format_names() {
            writeln!(out, "{name}")?;
        }
        return Ok(());
    }
    if args.list_output_formats {
        let mut out = std::io::stdout().lock();
        for name in format::output_format_names() {
            writeln!(out, "{name}")?;
        }
        return Ok(());
    }

    if let (Some(input), Some(output)) = (&args.input, &args.output) {
        if input == output {
            return Err(CliError::SameInputOutput);
        }
    }

    let input_format = format::resolve_format(
        args.from,
        args.input.as_deref(),
        "cannot detect input format: use --from",
    )?;
    let output_format = format::resolve_format(
        args.to,
        args.output.as_deref(),
        "cannot detect output format: use --to",
    )?;

    let use_stdin = args.input.as_deref().is_none_or(|p| p == Path::new("-"));

    let raw_content = if use_stdin {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        s
    } else {
        std::fs::read_to_string(args.input.as_deref().ok_or_else(|| {
            CliError::FormatDetection {
                message: "cannot read input".to_string(),
            }
        })?)?
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

    match &args.output {
        Some(path) => {
            let mut writer = BufWriter::new(File::create(path)?);
            run_pipeline(&content, &mut writer)?;
        }
        None => run_pipeline(&content, std::io::stdout().lock())?,
    }

    if args.verbose {
        let mut err = std::io::stderr().lock();
        writeln!(err, "conversion complete")?;
    }

    Ok(())
}

// Reason: Same as run_convert — single dispatch point by design.
#[allow(clippy::single_call_fn)]
fn run_http(args: &HttpArgs) -> Result<()> {
    docspec_http::tracing_init::init(&args.log_level, args.log_format.into())
        .map_err(|e| CliError::Io(std::io::Error::other(e.to_string())))?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime
        .block_on(docspec_http::serve(&args.host, args.port))
        .map_err(|e| CliError::Io(std::io::Error::other(e.to_string())))
}

/// Runs the streaming conversion pipeline from markdown to `BlockNote`.
///
/// Flushes the underlying writer before returning so I/O errors (disk full,
/// broken pipe, etc.) surface as a non-zero exit instead of being silently
/// dropped by `Drop`.
fn run_pipeline<W: Write>(content: &str, mut output: W) -> Result<()> {
    let reader = MarkdownReader::new(content);
    let sink = StackTrackingSink::new(BlockNoteWriter::new(&mut output));
    docspec_core::pipe(reader, sink)?;
    output.flush()?;
    Ok(())
}

/// Main entry point.
fn main() {
    let cli = Cli::parse();

    let result: Result<()> = match &cli.command {
        Command::Convert(args) => run_convert(args, cli.color),
        Command::Http(args) => run_http(args),
    };

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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]

    use super::*;

    struct FailingFlushWriter;

    impl Write for FailingFlushWriter {
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("simulated flush failure"))
        }
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(buf.len())
        }
    }

    #[test]
    fn run_pipeline_propagates_flush_error() {
        let result = run_pipeline("# Hello\n", FailingFlushWriter);
        let err = result.expect_err("expected flush error to propagate");
        assert!(
            matches!(
                err,
                CliError::Io(_) | CliError::Conversion(docspec_core::Error::Io { .. })
            ),
            "expected I/O-rooted error, got {err:?}"
        );
    }
}
