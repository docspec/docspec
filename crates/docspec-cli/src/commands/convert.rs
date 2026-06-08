//! `docspec convert` subcommand: convert documents between formats.

use std::fs::File;
use std::io::{BufWriter, Read as _, Write};

use docspec::{AnyReader, AnyWriter};

use crate::args::ConvertArgs;
use crate::error::{CliError, Result};
use crate::format;

/// Runs the streaming conversion pipeline.
fn run_pipeline<W: Write>(
    input_format: docspec::InputFormat,
    content: &str,
    output_format: docspec::OutputFormat,
    output: W,
) -> Result<()> {
    let reader = AnyReader::from_str(input_format, content);
    let sink = AnyWriter::new(output_format, output);
    docspec_core::pipe(reader, sink).map_err(Into::into)
}

fn write_cli_terminating_newline<W: Write>(output: &mut W) -> Result<()> {
    output.write_all(b"\n").map_err(Into::into)
}

/// Executes the `convert` subcommand.
pub fn run(args: ConvertArgs) -> Result<()> {
    let ConvertArgs {
        from,
        input: input_path,
        output: output_path,
        to,
        ..
    } = args;

    if let (Some(source_path), Some(destination_path)) = (&input_path, &output_path) {
        if source_path == destination_path {
            return Err(CliError::SameInputOutput);
        }
    }

    // Resolve formats BEFORE reading input (fail-fast)
    let input_format = format::resolve_input_format(
        from,
        input_path.as_deref(),
        "cannot detect input format: use --from",
    )?;
    let output_format = format::resolve_output_format(
        to,
        output_path.as_deref(),
        "cannot detect output format: use --to",
    )?;

    // Read input AFTER format validation
    let raw_content = match input_path.as_ref() {
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

    output_path.as_ref().map_or_else(
        || {
            let mut stdout = std::io::stdout().lock();
            run_pipeline(input_format, &content, output_format, &mut stdout)?;
            write_cli_terminating_newline(&mut stdout)
        },
        |path| {
            let mut writer = BufWriter::new(File::create(path)?);
            run_pipeline(input_format, &content, output_format, &mut writer)?;
            write_cli_terminating_newline(&mut writer)?;
            writer.flush()?;
            Ok(())
        },
    )
}
