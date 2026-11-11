//! `docspec convert` subcommand: convert documents between formats.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Cursor, ErrorKind, Read as _, Write};
use std::path::Path;

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
    let output_format = format::resolve_output_format(to, output_path.as_deref());

    let reader: AnyReader = match input_path.as_ref() {
        Some(path) if path.as_os_str() != "-" => {
            // Readers fail on a directory with format-specific kinds (EISDIR,
            // EINVAL); refuse it up front so every format exits EX_NOINPUT.
            if path.is_dir() {
                return Err(CliError::InputUnreadable {
                    path: path.clone(),
                    source: ErrorKind::IsADirectory.into(),
                });
            }
            // The reader reports an unopenable file as a generic I/O error;
            // keep the path so the exit code can follow the I/O kind.
            AnyReader::from_path(input_format, path).map_err(|error| match error {
                docspec_core::Error::Io { source } => CliError::InputUnreadable {
                    path: path.clone(),
                    source,
                },
                error => CliError::Conversion(error),
            })?
        }
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
            let unwritable = |source| CliError::OutputUnwritable {
                path: path.clone(),
                source,
            };
            let existing = std::fs::metadata(path).ok();
            let dangling_link = existing.is_none()
                && std::fs::symlink_metadata(path)
                    .is_ok_and(|metadata| metadata.file_type().is_symlink());
            if dangling_link
                || existing
                    .as_ref()
                    .is_some_and(|metadata| !metadata.is_file())
            {
                // Not a regular file (`/dev/stdout`, a FIFO, a directory, a
                // dangling symlink): there is nothing to keep and nowhere to
                // rename onto.
                let file = File::create(path).map_err(unwritable)?;
                return write_output(reader, output_format, file);
            }
            if existing.is_some() {
                // The rename would bypass the file's own write permission;
                // refuse a file `File::create` could not open either.
                OpenOptions::new()
                    .write(true)
                    .open(path)
                    .map_err(unwritable)?;
            }

            // Write beside the destination and rename only on success, so a
            // failed conversion leaves the previous output untouched. A
            // symlinked destination is resolved so the link itself survives.
            let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
            let dir = target
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            // `make_in` with `File::create_new` keeps the mode a fresh
            // `File::create` would get (not 0600) and reports the bare OS
            // error, without the temp file's path.
            let temp = tempfile::Builder::new()
                .make_in(dir, |temp_path| File::create_new(temp_path))
                .map_err(unwritable)?;
            if let Some(metadata) = existing {
                temp.as_file()
                    .set_permissions(metadata.permissions())
                    .map_err(unwritable)?;
            }
            write_output(reader, output_format, temp.as_file())?;
            temp.persist(&target)
                .map_err(|error| unwritable(error.error))?;
            Ok(())
        }
    }
}

fn write_output<W>(reader: AnyReader, output_format: docspec::OutputFormat, output: W) -> Result<()>
where
    W: Write,
{
    let mut writer = BufWriter::new(output);
    run_pipeline(reader, output_format, &mut writer)?;
    write_cli_terminating_newline(&mut writer)?;
    writer.flush()?;
    Ok(())
}
