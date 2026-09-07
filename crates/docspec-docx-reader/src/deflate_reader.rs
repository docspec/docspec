use std::io::{self, Read};

use flate2::{Decompress, FlushDecompress, Status};

const INPUT_CAPACITY: usize = 32 * 1024;

pub(crate) struct DeflateReader<R> {
    inner: R,
    decoder: Decompress,
    input: Box<[u8]>,
    input_start: usize,
    input_end: usize,
    finished: bool,
}

impl<R> DeflateReader<R>
where
    R: Read,
{
    pub(crate) fn new(inner: R) -> Self {
        Self {
            inner,
            decoder: Decompress::new(false),
            input: vec![0; INPUT_CAPACITY].into_boxed_slice(),
            input_start: 0,
            input_end: 0,
            finished: false,
        }
    }

    pub(crate) fn total_in(&self) -> u64 {
        self.decoder.total_in()
    }

    fn refill(&mut self) -> io::Result<bool> {
        let count = self.inner.read(self.input.as_mut())?;
        self.input_start = 0;
        self.input_end = count;
        Ok(count != 0)
    }

    fn decode(&mut self, output: &mut [u8], flush: FlushDecompress) -> io::Result<(usize, Status)> {
        let input = self
            .input
            .get(self.input_start..self.input_end)
            .ok_or_else(|| io::Error::other("invalid deflate input range"))?;
        let before_in = self.decoder.total_in();
        let before_out = self.decoder.total_out();
        let status = self
            .decoder
            .decompress(input, output, flush)
            .map_err(|_error| {
                io::Error::new(io::ErrorKind::InvalidData, "corrupt deflate stream")
            })?;
        let consumed = usize::try_from(self.decoder.total_in().saturating_sub(before_in))
            .map_err(io::Error::other)?;
        let produced = usize::try_from(self.decoder.total_out().saturating_sub(before_out))
            .map_err(io::Error::other)?;
        self.input_start = self
            .input_start
            .checked_add(consumed)
            .ok_or_else(|| io::Error::other("deflate input position overflow"))?;
        Ok((produced, status))
    }

    fn finish(&mut self, produced: usize) -> usize {
        self.finished = true;
        produced
    }
}

fn no_progress_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "deflate decoder made no progress",
    )
}

impl<R> Read for DeflateReader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.finished {
            return Ok(0);
        }
        loop {
            if self.input_start == self.input_end && !self.refill()? {
                let (produced, status) = self.decode(buf, FlushDecompress::Finish)?;
                if status == Status::StreamEnd {
                    return Ok(self.finish(produced));
                }
                if produced == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "truncated deflate stream",
                    ));
                }
                return Ok(produced);
            }
            let before = self.input_start;
            let (produced, status) = self.decode(buf, FlushDecompress::None)?;
            if status == Status::StreamEnd {
                return Ok(self.finish(produced));
            }
            if produced != 0 {
                return Ok(produced);
            }
            if self.input_start == before {
                return Err(no_progress_error());
            }
        }
    }
}

#[cfg(test)]
#[path = "deflate_reader_tests.rs"]
mod tests;
