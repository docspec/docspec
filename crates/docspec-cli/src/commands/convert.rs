//! `docspec convert` subcommand: convert documents between formats.

use std::fs::File;
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

/// Whether `a` and `b` name the same existing file, however they are spelled:
/// `./x` and `x`, absolute and relative, symlinks, and (on Unix) hard links.
/// A path that does not exist cannot be the other one. Any other lookup error
/// is returned rather than read as "different files".
fn same_file(a: &Path, b: &Path) -> Result<bool> {
    #[cfg(unix)]
    fn identity(path: &Path) -> std::io::Result<(u64, u64)> {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata(path).map(|meta| (meta.dev(), meta.ino()))
    }
    #[cfg(not(unix))]
    fn identity(path: &Path) -> std::io::Result<std::path::PathBuf> {
        std::fs::canonicalize(path)
    }
    let a_id = match identity(a) {
        Ok(id) => id,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(source) => {
            return Err(CliError::InputUnreadable {
                path: a.to_path_buf(),
                source,
            })
        }
    };
    match identity(b) {
        Ok(b_id) => Ok(a_id == b_id),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(source) => Err(CliError::OutputUnwritable {
            path: b.to_path_buf(),
            source,
        }),
    }
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
        // The literal comparison also refuses a path that does not exist yet.
        if source_path.as_os_str() != "-"
            && (source_path == destination_path || same_file(source_path, destination_path)?)
        {
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
            let file = File::create(path).map_err(|source| CliError::OutputUnwritable {
                path: path.clone(),
                source,
            })?;
            let mut writer = BufWriter::new(file);
            run_pipeline(reader, output_format, &mut writer)?;
            write_cli_terminating_newline(&mut writer)?;
            writer.flush()?;
            Ok(())
        }
    }
}
