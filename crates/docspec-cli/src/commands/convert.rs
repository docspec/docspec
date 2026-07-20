//! `docspec convert` subcommand: convert documents between formats.

use std::fs::File;
use std::io::{BufWriter, Cursor, Read as _, Write};

use docspec::{AnyReader, AnyWriter};

use crate::args::ConvertArgs;
use crate::error::{CliError, Result};
use crate::format;

fn run_pipeline<W>(reader: AnyReader, output_format: docspec::OutputFormat, output: W) -> Result<()>
where
    W: Write,
{
    let sink = AnyWriter::new(output_format, output);
    docspec_core::pipe(docspec_core::SkipEmptyBlocks::new(reader), sink).map_err(Into::into)
}

fn write_cli_terminating_newline<W>(output: &mut W) -> Result<()>
where
    W: Write,
{
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

    let reader: AnyReader = match input_path.as_ref() {
        Some(path) if path.as_os_str() != "-" => AnyReader::from_path(input_format, path)?,
        _ => {
            let mut buf = Vec::new();
            std::io::stdin().lock().read_to_end(&mut buf)?;
            AnyReader::from_reader(input_format, Cursor::new(buf))?
        }
    };

    match output_path.as_ref() {
        None => {
            let mut stdout = std::io::stdout().lock();
            run_pipeline(reader, output_format, &mut stdout)?;
            write_cli_terminating_newline(&mut stdout)
        }
        Some(path) => {
            let mut writer = BufWriter::new(File::create(path)?);
            run_pipeline(reader, output_format, &mut writer)?;
            write_cli_terminating_newline(&mut writer)?;
            writer.flush()?;
            Ok(())
        }
    }
}
