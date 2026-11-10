#![forbid(unsafe_code)]
//! ZIP container access shared by the `DocSpec` readers for ZIP-based formats.
//!
//! DOCX and ODT are both ZIP archives of XML parts. This crate holds what their
//! readers have in common:
//!
//! - [`open_archive`] opens an archive and maps failures to [`docspec_core::Error`].
//! - [`read_optional_entry`] reads one small entry into memory.
//! - [`StreamingArchive`] streams one large entry from a file in constant memory.

mod streaming_archive;

use std::io::{Read, Seek};

use docspec_core::{Error, Result};
use zip::result::ZipError;
use zip::ZipArchive;

pub use streaming_archive::StreamingArchive;

/// Object-safe alias combining [`Read`], [`Seek`], and [`Send`] for use in trait objects.
pub trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

/// A ZIP archive over any boxed [`ReadSeek`] source.
pub type Archive = ZipArchive<Box<dyn ReadSeek + 'static>>;

/// Opens the ZIP archive in `reader`.
///
/// # Errors
///
/// Returns [`Error::Parse`] if the input is not a valid ZIP archive and
/// [`Error::Io`] if reading it fails.
#[inline]
pub fn open_archive<R>(reader: R) -> Result<Archive>
where
    R: Read + Seek + Send + 'static,
{
    let boxed: Box<dyn ReadSeek + 'static> = Box::new(reader);
    ZipArchive::new(boxed).map_err(map_zip_open_error)
}

fn map_zip_open_error(err: ZipError) -> Error {
    match err {
        ZipError::InvalidArchive(_) | ZipError::UnsupportedArchive(_) => Error::Parse {
            message: "not a valid ZIP archive".to_string(),
            position: None,
        },
        ZipError::Io(source) => Error::Io { source },
        ZipError::FileNotFound
        | ZipError::InvalidPassword
        | ZipError::CompressionMethodNotSupported(_)
        | _ => Error::Parse {
            message: format!("not a valid ZIP archive: {err}"),
            position: None,
        },
    }
}

/// Reads the entry named `path` into memory; `None` when the archive has no such entry.
///
/// # Errors
///
/// Returns [`Error::Parse`] if the archive is malformed and [`Error::Io`] if
/// reading the entry fails.
#[inline]
pub fn read_optional_entry<R>(archive: &mut ZipArchive<R>, path: &str) -> Result<Option<Vec<u8>>>
where
    R: Read + Seek,
{
    match archive.by_name(path) {
        Ok(mut entry) => {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(Error::from)?;
            Ok(Some(bytes))
        }
        Err(ZipError::FileNotFound) => Ok(None),
        Err(err) => Err(Error::Parse {
            message: format!("malformed ZIP: {err}"),
            position: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::unwrap_used)]

    use std::io::{Cursor, Write as _};

    use super::*;

    fn zip_with_entry(name: &str, content: &[u8]) -> Cursor<Vec<u8>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
        Cursor::new(writer.finish().unwrap().into_inner())
    }

    #[test]
    fn read_optional_entry_returns_bytes_or_none() {
        let mut archive = open_archive(zip_with_entry("content.xml", b"<a/>")).unwrap();
        assert_eq!(
            read_optional_entry(&mut archive, "content.xml").unwrap(),
            Some(b"<a/>".to_vec())
        );
        assert_eq!(
            read_optional_entry(&mut archive, "missing.xml").unwrap(),
            None
        );
    }

    #[test]
    fn open_archive_rejects_non_zip_input() {
        let Err(Error::Parse { message, position }) = open_archive(Cursor::new(b"nope".to_vec()))
        else {
            panic!("expected a parse error");
        };
        assert_eq!(message, "not a valid ZIP archive");
        assert_eq!(position, None);
    }
}
