use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

use crate::package::ReadSeek;

pub(crate) struct SharedReader {
    inner: Arc<Mutex<Box<dyn ReadSeek + 'static>>>,
}

impl SharedReader {
    pub(crate) fn new<R>(reader: R) -> Self
    where
        R: Read + Seek + Send + 'static,
    {
        Self {
            inner: Arc::new(Mutex::new(Box::new(reader))),
        }
    }

    pub(crate) fn cursor(&self) -> SharedCursor {
        SharedCursor {
            inner: Arc::clone(&self.inner),
            position: 0,
        }
    }
}

pub(crate) struct SharedCursor {
    inner: Arc<Mutex<Box<dyn ReadSeek + 'static>>>,
    position: u64,
}

impl Read for SharedCursor {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut reader = self
            .inner
            .lock()
            .map_err(|_error| io::Error::other("DOCX source mutex poisoned"))?;
        reader.seek(SeekFrom::Start(self.position))?;
        let count = reader.read(buf)?;
        let count_u64 = u64::try_from(count).map_err(io::Error::other)?;
        self.position = self
            .position
            .checked_add(count_u64)
            .ok_or_else(|| io::Error::other("DOCX source cursor overflow"))?;
        Ok(count)
    }
}

impl Seek for SharedCursor {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let next = match pos {
            SeekFrom::Start(position) => position,
            SeekFrom::Current(offset) => checked_offset(self.position, offset)?,
            SeekFrom::End(offset) => {
                let mut reader = self
                    .inner
                    .lock()
                    .map_err(|_error| io::Error::other("DOCX source mutex poisoned"))?;
                let end = reader.seek(SeekFrom::End(0))?;
                checked_offset(end, offset)?
            }
        };
        self.position = next;
        Ok(next)
    }
}

fn checked_offset(base: u64, offset: i64) -> io::Result<u64> {
    base.checked_add_signed(offset)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid seek"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use std::io::Cursor;

    #[test]
    fn cursors_seek_and_read_independently() {
        let source = SharedReader::new(Cursor::new(b"abcdef".to_vec()));
        let mut first = source.cursor();
        let mut second = source.cursor();
        first.seek(SeekFrom::Start(2)).expect("seek first");
        second.seek(SeekFrom::Start(4)).expect("seek second");
        let mut first_bytes = [0; 2];
        let mut second_bytes = [0; 2];
        first.read_exact(&mut first_bytes).expect("read first");
        second.read_exact(&mut second_bytes).expect("read second");
        assert_eq!(first_bytes, *b"cd");
        assert_eq!(second_bytes, *b"ef");
        assert_eq!(first.stream_position().expect("first position"), 4);
        assert_eq!(second.stream_position().expect("second position"), 6);
    }

    #[test]
    fn cursor_rejects_seek_before_start() {
        let source = SharedReader::new(Cursor::new(Vec::<u8>::new()));
        let mut cursor = source.cursor();
        let error = cursor
            .seek(SeekFrom::Current(-1))
            .expect_err("negative position must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(error.to_string(), "invalid seek");
    }
}
