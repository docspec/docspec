//! Internal helper: wraps a `Read + Seek` source and silently consumes a
//! leading UTF-8 byte-order mark (U+FEFF, bytes `0xEF 0xBB 0xBF`) on the
//! first read, leaving the position immediately after the BOM. If no BOM
//! is present, the reader rewinds to the start so the caller sees the
//! original bytes unchanged.
//!
//! Private to the facade — used by [`crate::factory::reader::AnyReader::from_reader`]
//! to apply BOM-stripping uniformly across all text-format readers without
//! requiring each concrete reader to implement BOM handling.

use std::io::{Read, Seek, SeekFrom};

use docspec_core::Result;

/// Detects and skips a leading UTF-8 BOM on the wrapped reader.
///
/// Construction performs the BOM check eagerly via [`Self::new`].
/// After construction, the reader is positioned either just past the BOM
/// (if present) or back at byte 0 (if no BOM was found). All subsequent
/// `Read` operations transparently forward to the inner reader.
#[derive(Debug)]
pub(in crate::factory) struct BomStrippingReader<R: Read + Seek> {
    inner: R,
}

impl<R: Read + Seek> BomStrippingReader<R> {
    /// Wraps `reader` and consumes a leading UTF-8 BOM if present.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the underlying reader returns an I/O error during the
    /// BOM probe or the subsequent seek-back.
    pub(in crate::factory) fn new(mut reader: R) -> Result<Self> {
        let mut probe = [u8::default(); 3];
        let start = reader
            .stream_position()
            .map_err(|source| docspec_core::Error::Io { source })?;
        let read_count = read_up_to_3(&mut reader, &mut probe)
            .map_err(|source| docspec_core::Error::Io { source })?;
        let has_bom = read_count == 3 && probe == [0xEF, 0xBB, 0xBF];
        if !has_bom {
            // Rewind to the original position so the caller sees every byte.
            reader
                .seek(SeekFrom::Start(start))
                .map_err(|source| docspec_core::Error::Io { source })?;
        }
        Ok(Self { inner: reader })
    }
}

impl<R: Read + Seek> Read for BomStrippingReader<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<R: Read + Seek> Seek for BomStrippingReader<R> {
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

/// Reads up to 3 bytes into `buf`, handling short reads gracefully.
///
/// Returns the number of bytes actually read (0, 1, 2, or 3). Unlike
/// `read_exact`, this never returns `UnexpectedEof` for inputs shorter than
/// 3 bytes — those are valid text (e.g. empty input, one character).
fn read_up_to_3<R: Read>(reader: &mut R, buf: &mut [u8; 3]) -> std::io::Result<usize> {
    let mut filled = usize::default();
    while filled < 3 {
        let remaining = buf.get_mut(filled..).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "BOM probe index exceeds buffer length",
            )
        })?;
        match reader.read(remaining) {
            Ok(0) => break,
            Ok(n) => filled = filled.saturating_add(n),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::io::{Cursor, Read as _};

    use super::*;

    #[test]
    fn strips_bom_when_present() {
        let mut reader = BomStrippingReader::new(Cursor::new(b"\xEF\xBB\xBF# Hello".to_vec()))
            .expect("BOM probe succeeds");
        let mut out = String::new();
        reader.read_to_string(&mut out).expect("read succeeds");
        assert_eq!(out, "# Hello");
    }

    #[test]
    fn passes_through_when_no_bom() {
        let mut reader =
            BomStrippingReader::new(Cursor::new(b"# Hello".to_vec())).expect("probe succeeds");
        let mut out = String::new();
        reader.read_to_string(&mut out).expect("read succeeds");
        assert_eq!(out, "# Hello");
    }

    #[test]
    fn handles_empty_input() {
        let mut reader = BomStrippingReader::new(Cursor::new(Vec::<u8>::new())).expect("probe");
        let mut out = String::new();
        reader.read_to_string(&mut out).expect("read");
        assert_eq!(out, "");
    }

    #[test]
    fn handles_short_input_no_bom() {
        let mut reader =
            BomStrippingReader::new(Cursor::new(b"hi".to_vec())).expect("probe succeeds");
        let mut out = String::new();
        reader.read_to_string(&mut out).expect("read");
        assert_eq!(out, "hi");
    }

    #[test]
    fn handles_bom_only_input() {
        let mut reader =
            BomStrippingReader::new(Cursor::new(b"\xEF\xBB\xBF".to_vec())).expect("probe succeeds");
        let mut out = String::new();
        reader.read_to_string(&mut out).expect("read");
        assert_eq!(out, "");
    }

    #[test]
    fn handles_partial_bom_sequence() {
        // Bytes that look like start of BOM but are not — must NOT strip
        let mut reader =
            BomStrippingReader::new(Cursor::new(b"\xEF\xBB X".to_vec())).expect("probe succeeds");
        let mut out = Vec::new();
        reader.read_to_end(&mut out).expect("read succeeds");
        assert_eq!(out, b"\xEF\xBB X");
    }
}
