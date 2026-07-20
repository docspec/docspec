//! Streaming reader for a single ZIP entry without buffering it in memory.
//!
//! `StreamingArchive::open` uses `ZipArchive` only to locate an entry and capture
//! its compressed byte range. The returned reader then streams that byte range
//! directly from the DOCX file with bounded memory.

use std::fs::File;
use std::io::{Read, Seek as _, SeekFrom};
use std::path::Path;

use docspec_core::{Error, Result};
use flate2::read::DeflateDecoder;
use zip::{result::ZipError, CompressionMethod, ZipArchive};

enum EntryReader {
    Stored(std::io::Take<File>),
    Deflated(DeflateDecoder<std::io::Take<File>>),
}

impl Read for EntryReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Stored(reader) => reader.read(buf),
            Self::Deflated(reader) => reader.read(buf),
        }
    }
}

/// Streaming reader for one DOCX ZIP entry.
///
/// Constructed via [`StreamingArchive::open`]. The archive is inspected once for
/// entry metadata; `Read` then pulls bytes directly from the compressed entry
/// range without materializing the full XML document.
pub(crate) struct StreamingArchive {
    reader: EntryReader,
}

impl StreamingArchive {
    /// Opens the ZIP archive at `path` and positions the reader on the entry
    /// named `entry_name` (typically `"word/document.xml"`).
    pub(crate) fn open(path: &Path, entry_name: &str) -> Result<Self> {
        let file = File::open(path).map_err(Error::from)?;
        let mut archive = ZipArchive::new(file).map_err(|err| match err {
            ZipError::Io(source) => Error::Io { source },
            other => Error::Parse {
                message: format!("not a valid ZIP archive: {other}"),
                position: None,
            },
        })?;

        let entry = archive.by_name(entry_name).map_err(|err| match err {
            ZipError::Io(source) => Error::Io { source },
            other => Error::Parse {
                message: format!("document target not found: {other}"),
                position: None,
            },
        })?;
        let data_start = entry.data_start().ok_or_else(|| Error::Parse {
            message: format!("document target has no data offset: {entry_name}"),
            position: None,
        })?;
        let compressed_size = entry.compressed_size();
        let compression = entry.compression();
        drop(entry);

        let mut stream_file = archive.into_inner();
        stream_file
            .seek(SeekFrom::Start(data_start))
            .map_err(Error::from)?;
        let compressed_reader = stream_file.take(compressed_size);
        let reader = match compression {
            CompressionMethod::Stored => EntryReader::Stored(compressed_reader),
            CompressionMethod::Deflated => {
                EntryReader::Deflated(DeflateDecoder::new(compressed_reader))
            }
            _ => {
                return Err(Error::Parse {
                    message: format!("unsupported ZIP compression method: {compression:?}"),
                    position: None,
                });
            }
        };

        Ok(Self { reader })
    }
}

impl Read for StreamingArchive {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}

// Compile-time assertion: StreamingArchive must be Send + 'static.
const _: fn(&Path, &str) -> Result<StreamingArchive> = StreamingArchive::open;

const _: fn() = || {
    fn assert_send<T>()
    where
        T: Send + 'static,
    {
    }
    assert_send::<StreamingArchive>();
};

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use std::io::Write as _;

    fn make_test_docx_with_entry(entry_name: &str, content: &[u8]) -> tempfile::NamedTempFile {
        let tmp = tempfile::NamedTempFile::new().expect("tempfile");
        let buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer.start_file(entry_name, options).expect("start_file");
        writer.write_all(content).expect("write_all");
        let zip_bytes = writer.finish().expect("finish").into_inner();
        let mut file = tmp.reopen().expect("reopen");
        file.write_all(&zip_bytes).expect("write to tempfile");
        drop(file);
        tmp
    }

    #[test]
    fn streaming_archive_reads_entry_content() {
        let xml_content = b"<?xml version=\"1.0\"?><root>hello</root>";
        let tmp = make_test_docx_with_entry("word/document.xml", xml_content);
        let mut archive = StreamingArchive::open(tmp.path(), "word/document.xml").unwrap();
        let mut buf = Vec::new();
        archive.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, xml_content);
    }

    #[test]
    fn streaming_archive_open_missing_entry_returns_error() {
        let tmp = make_test_docx_with_entry("word/document.xml", b"<?xml?>");
        let result = StreamingArchive::open(tmp.path(), "word/missing.xml");
        assert!(result.is_err());
    }
}
