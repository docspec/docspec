//! Streaming reader for a single ZIP entry without buffering it in memory.

use std::fs::File;
use std::io::{self, Read, Seek as _, SeekFrom};
use std::path::Path;

use crc32fast::Hasher;
use docspec_core::{Error, Result};
use flate2::read::DeflateDecoder;
use zip::{result::ZipError, CompressionMethod, ZipArchive};

use crate::package::ReadSeek;

#[derive(Clone, Copy)]
struct EntryMetadata {
    data_start: u64,
    compressed_size: u64,
    uncompressed_size: u64,
    crc32: u32,
    compression: CompressionMethod,
}

enum EntryReader {
    Stored(std::io::Take<Box<dyn ReadSeek + 'static>>),
    DeflatedLegacy(DeflateDecoder<std::io::Take<Box<dyn ReadSeek + 'static>>>),
    DeflatedStrict(
        crate::deflate_reader::DeflateReader<std::io::Take<Box<dyn ReadSeek + 'static>>>,
    ),
}

impl Read for EntryReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Stored(reader) => reader.read(buf),
            Self::DeflatedLegacy(reader) => reader.read(buf),
            Self::DeflatedStrict(reader) => reader.read(buf),
        }
    }
}

pub(crate) struct StreamingArchive {
    reader: EntryReader,
    compressed_size: u64,
    uncompressed_size: u64,
    bytes_read: u64,
    expected_crc32: u32,
    crc32: Hasher,
    verified: bool,
    verify_integrity: bool,
}

impl StreamingArchive {
    pub(crate) fn open(path: &Path, entry_name: &str) -> Result<Self> {
        let file = File::open(path).map_err(Error::from)?;
        let mut archive = ZipArchive::new(file).map_err(map_archive_error)?;
        let metadata = entry_metadata(&mut archive, entry_name)?;
        let source: Box<dyn ReadSeek + 'static> = Box::new(archive.into_inner());
        Self::from_source(source, metadata, false)
    }

    pub(crate) fn open_from_archive<R>(
        source: R,
        archive: &mut ZipArchive<Box<dyn ReadSeek + 'static>>,
        entry_name: &str,
    ) -> Result<Self>
    where
        R: Read + std::io::Seek + Send + 'static,
    {
        let metadata = entry_metadata(archive, entry_name)?;
        Self::from_source(Box::new(source), metadata, true)
    }

    fn from_source(
        mut source: Box<dyn ReadSeek + 'static>,
        metadata: EntryMetadata,
        verify_integrity: bool,
    ) -> Result<Self> {
        if verify_integrity {
            let source_length = source.seek(SeekFrom::End(0)).map_err(Error::from)?;
            let data_end = metadata
                .data_start
                .checked_add(metadata.compressed_size)
                .ok_or_else(|| parse_error("ZIP entry boundary overflow"))?;
            if data_end > source_length {
                return Err(parse_error("ZIP entry exceeds archive boundary"));
            }
        }
        source
            .seek(SeekFrom::Start(metadata.data_start))
            .map_err(Error::from)?;
        let limited = source.take(metadata.compressed_size);
        let reader = match metadata.compression {
            CompressionMethod::Stored => EntryReader::Stored(limited),
            CompressionMethod::Deflated if verify_integrity => {
                EntryReader::DeflatedStrict(crate::deflate_reader::DeflateReader::new(limited))
            }
            CompressionMethod::Deflated => {
                EntryReader::DeflatedLegacy(DeflateDecoder::new(limited))
            }
            compression => {
                return Err(parse_error(format!(
                    "unsupported ZIP compression method: {compression:?}"
                )))
            }
        };
        Ok(Self {
            reader,
            compressed_size: metadata.compressed_size,
            uncompressed_size: metadata.uncompressed_size,
            bytes_read: 0,
            expected_crc32: metadata.crc32,
            crc32: Hasher::new(),
            verified: false,
            verify_integrity,
        })
    }

    fn verify_end(&mut self) -> io::Result<()> {
        if self.bytes_read != self.uncompressed_size {
            return Err(invalid_data(format!(
                "ZIP entry length mismatch: expected {}, read {}",
                self.uncompressed_size, self.bytes_read
            )));
        }
        let compressed_read = match &self.reader {
            EntryReader::Stored(reader) => self
                .compressed_size
                .checked_sub(reader.limit())
                .ok_or_else(|| invalid_data("ZIP compressed length underflow"))?,
            EntryReader::DeflatedLegacy(reader) => reader.total_in(),
            EntryReader::DeflatedStrict(reader) => reader.total_in(),
        };
        if compressed_read != self.compressed_size {
            return Err(invalid_data(format!(
                "ZIP compressed length mismatch: expected {}, read {compressed_read}",
                self.compressed_size
            )));
        }
        let actual_crc32 = self.crc32.clone().finalize();
        if actual_crc32 != self.expected_crc32 {
            return Err(invalid_data(format!(
                "ZIP entry CRC mismatch: expected {:08x}, calculated {actual_crc32:08x}",
                self.expected_crc32
            )));
        }
        self.verified = true;
        Ok(())
    }
}

impl Read for StreamingArchive {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.verified || buf.is_empty() {
            return Ok(0);
        }
        let count = self.reader.read(buf)?;
        if count == 0 {
            if self.verify_integrity {
                self.verify_end()?;
            } else {
                self.verified = true;
            }
            return Ok(0);
        }
        if !self.verify_integrity {
            return Ok(count);
        }
        let count_u64 = u64::try_from(count).map_err(io::Error::other)?;
        let next = self
            .bytes_read
            .checked_add(count_u64)
            .ok_or_else(|| invalid_data("ZIP entry length overflow"))?;
        if next > self.uncompressed_size {
            return Err(invalid_data(format!(
                "ZIP entry exceeds declared length {}",
                self.uncompressed_size
            )));
        }
        let bytes = buf
            .get(..count)
            .ok_or_else(|| invalid_data("ZIP reader returned an invalid byte count"))?;
        self.crc32.update(bytes);
        self.bytes_read = next;
        Ok(count)
    }
}

fn entry_metadata<R>(archive: &mut ZipArchive<R>, entry_name: &str) -> Result<EntryMetadata>
where
    R: Read + std::io::Seek,
{
    let entry = archive.by_name(entry_name).map_err(|error| match error {
        ZipError::Io(source) => Error::Io { source },
        other => parse_error(format!("document target not found: {other}")),
    })?;
    let data_start = entry
        .data_start()
        .ok_or_else(|| parse_error(format!("document target has no data offset: {entry_name}")))?;
    Ok(EntryMetadata {
        data_start,
        compressed_size: entry.compressed_size(),
        uncompressed_size: entry.size(),
        crc32: entry.crc32(),
        compression: entry.compression(),
    })
}

fn map_archive_error(error: ZipError) -> Error {
    match error {
        ZipError::Io(source) => Error::Io { source },
        other => parse_error(format!("not a valid ZIP archive: {other}")),
    }
}

fn parse_error(message: impl Into<String>) -> Error {
    Error::Parse {
        message: message.into(),
        position: None,
    }
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

const _: fn(&Path, &str) -> Result<StreamingArchive> = StreamingArchive::open;

#[cfg(test)]
#[path = "streaming_archive_tests.rs"]
mod tests;
