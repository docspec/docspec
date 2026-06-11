//! A configurable [`std::io::Write`] mock that fails after a threshold of
//! successful writes.
//!
//! Used by writer integration tests to verify error propagation. Wraps the
//! writer-under-test around a [`FailingWriter`] configured to fail on the Nth
//! write, then assert that the error surfaces correctly through the writer's
//! [`docspec_core::EventSink`] implementation.

use std::io::{self, Write};

/// A [`std::io::Write`] implementation that fails after a configurable number
/// of successful writes.
///
/// Construct with [`FailingWriter::new`] to specify how many writes succeed
/// before failures begin. Use [`FailingWriter::with_kind`] to override the
/// [`io::ErrorKind`] returned on failure (defaults to [`io::ErrorKind::Other`]).
///
/// [`FailingWriter::flush`] always succeeds; only [`FailingWriter::write`]
/// participates in the threshold counter. Failure responses always use the
/// message `"simulated write failure"`.
///
/// # Examples
///
/// Fail immediately on the first write:
///
/// ```
/// use docspec_test_utils::FailingWriter;
/// use std::io::Write;
///
/// let mut writer = FailingWriter::new(0);
/// assert!(writer.write(b"data").is_err());
/// ```
///
/// Succeed once, then fail with a [`io::ErrorKind::BrokenPipe`] error:
///
/// ```
/// use docspec_test_utils::FailingWriter;
/// use std::io::{ErrorKind, Write};
///
/// let mut writer = FailingWriter::new(1).with_kind(ErrorKind::BrokenPipe);
/// assert!(writer.write(b"first").is_ok());
/// let err = writer.write(b"second").expect_err("second write must fail");
/// assert_eq!(err.kind(), ErrorKind::BrokenPipe);
/// ```
#[derive(Debug)]
pub struct FailingWriter {
    fail_after: usize,
    kind: io::ErrorKind,
    writes: usize,
}

impl FailingWriter {
    /// Constructs a [`FailingWriter`] that succeeds for the first `fail_after`
    /// writes and fails on every write thereafter.
    ///
    /// Pass `0` to fail immediately on the first write. The default error kind
    /// is [`io::ErrorKind::Other`]; use [`FailingWriter::with_kind`] to
    /// override.
    #[inline]
    #[must_use]
    pub fn new(fail_after: usize) -> Self {
        Self {
            fail_after,
            kind: io::ErrorKind::Other,
            writes: 0,
        }
    }

    /// Overrides the [`io::ErrorKind`] returned by failing writes.
    ///
    /// Defaults to [`io::ErrorKind::Other`].
    #[inline]
    #[must_use]
    pub fn with_kind(mut self, kind: io::ErrorKind) -> Self {
        self.kind = kind;
        self
    }
}

impl Write for FailingWriter {
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writes = self.writes.saturating_add(1);
        if self.writes > self.fail_after {
            return Err(io::Error::new(self.kind, "simulated write failure"));
        }
        Ok(buf.len())
    }
}
