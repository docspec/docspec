//! Input loading for the `DocSpec` CLI.
//!
//! File paths are memory-mapped so resident memory is controlled by the OS
//! working set rather than the full input size. Standard input is buffered in a
//! `String` because pipes cannot be memory-mapped.

use std::io::Read as _;
use std::path::Path;

use docspec_core::{Error, Result};

/// Loaded CLI input with owned backing storage.
///
/// Call [`LoadedInput::as_str`] to borrow the BOM-stripped UTF-8 content.
pub struct LoadedInput {
    inner: LoadedInner,
}

enum LoadedInner {
    Buffered {
        data: String,
        bom_offset: usize,
    },
    Mapped {
        mmap: memmap2::Mmap,
        bom_offset: usize,
    },
}

impl LoadedInput {
    /// Returns the BOM-stripped input content as a borrowed string slice.
    ///
    /// File inputs slice directly into the memory map. Standard input slices
    /// into the already-read buffer without an additional allocation.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        match &self.inner {
            LoadedInner::Buffered { data, bom_offset } => {
                data.get(*bom_offset..).unwrap_or_default()
            }
            LoadedInner::Mapped { mmap, bom_offset } => mmap
                .get(*bom_offset..)
                .and_then(|bytes| core::str::from_utf8(bytes).ok())
                .unwrap_or_default(),
        }
    }
}

/// Loads input from a file path or standard input.
///
/// Passing `None` or `Some("-")` reads standard input into a `String`. Passing a
/// real file path opens and memory-maps the file, then validates UTF-8 before
/// returning.
///
/// # Errors
///
/// Returns an error if standard input or the file path cannot be read, or if a
/// file path does not contain valid UTF-8.
#[allow(unsafe_code)]
#[inline]
pub fn load_input(input_path: Option<&Path>) -> Result<LoadedInput> {
    match input_path {
        None => load_stdin(),
        Some(path_value) if path_value.as_os_str() == "-" => load_stdin(),
        Some(path_value) => {
            let file = std::fs::File::open(path_value).map_err(|source| Error::Io { source })?;
            // SAFETY: The mapping is read-only and is owned by LoadedInput for the
            // duration of all borrows returned from as_str. UTF-8 validity is checked
            // before construction, and the CLI accepts the usual mmap trade-off that
            // external file truncation can fault the process.
            let mmap =
                unsafe { memmap2::Mmap::map(&file) }.map_err(|source| Error::Io { source })?;

            let raw = core::str::from_utf8(&mmap).map_err(|err| Error::Parse {
                message: format!("input is not valid UTF-8: {err}"),
                position: None,
            })?;

            let bom_offset = bom_offset_for_str(raw);
            Ok(LoadedInput {
                inner: LoadedInner::Mapped { mmap, bom_offset },
            })
        }
    }
}

fn load_stdin() -> Result<LoadedInput> {
    let mut data = String::new();
    std::io::stdin()
        .lock()
        .read_to_string(&mut data)
        .map_err(|source| Error::Io { source })?;
    let bom_offset = bom_offset_for_str(&data);
    Ok(LoadedInput {
        inner: LoadedInner::Buffered { data, bom_offset },
    })
}

fn bom_offset_for_str(data: &str) -> usize {
    if data.strip_prefix('\u{FEFF}').is_some() {
        '\u{FEFF}'.len_utf8()
    } else {
        0
    }
}
