#![warn(missing_docs)]
//! Library interface for `docspec-cli`.
//!
//! Exposes the input loader and streaming pipeline driver for integration tests.

mod args;
mod error;
mod format;

pub mod input;

use std::io::Write;

use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::{EventSink as _, EventSource as _, StackTrackingSink};
use docspec_markdown_reader::MarkdownReader;

pub use args::{Cli, ColorChoice, Format};
pub use error::{CliError, Result};
pub use format::resolve_format;
pub use input::{load_input, LoadedInput};

/// Runs the streaming conversion pipeline from markdown to `BlockNote`.
///
/// # Errors
///
/// Returns an error if the markdown reader, stack validator, or `BlockNote`
/// writer reports a conversion or I/O failure.
#[inline]
pub fn run_pipeline<W: Write>(content: &str, output: W) -> Result<()> {
    let mut reader = MarkdownReader::new(content);
    let mut sink = StackTrackingSink::new(BlockNoteWriter::new(output));

    let mut next = reader.next_event();
    while let Ok(Some(event)) = next {
        sink.handle_event(event)?;
        next = reader.next_event();
    }
    if let Err(err) = next {
        return Err(err.into());
    }
    sink.finish()?;
    Ok(())
}
