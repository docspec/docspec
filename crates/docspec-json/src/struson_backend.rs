//! [`JsonBackend`] adapter for `struson::JsonStreamWriter`.

use std::io::Write;

use struson::writer::{JsonStreamWriter, JsonWriter as _};

use crate::JsonBackend;
use docspec_core::{Error, Result};

/// Adapter wrapping `struson::JsonStreamWriter<W>`.
///
/// Forwards each [`JsonBackend`] method to the corresponding struson API call.
/// Errors from struson are converted to [`docspec_core::Error::Io`].
pub struct StrusonBackend<W: Write> {
    writer: JsonStreamWriter<W>,
}

impl<W: Write> StrusonBackend<W> {
    /// Create a new `StrusonBackend` wrapping the given writer.
    #[inline]
    pub fn new(writer: W) -> Self {
        Self {
            writer: JsonStreamWriter::new(writer),
        }
    }
}

impl<W: Write> JsonBackend for StrusonBackend<W> {
    type Output = W;

    #[inline]
    fn begin_array(&mut self) -> Result<()> {
        self.writer.begin_array().map_err(Error::from)
    }

    #[inline]
    fn begin_object(&mut self) -> Result<()> {
        self.writer.begin_object().map_err(Error::from)
    }

    #[inline]
    fn end_array(&mut self) -> Result<()> {
        self.writer.end_array().map_err(Error::from)
    }

    #[inline]
    fn end_object(&mut self) -> Result<()> {
        self.writer.end_object().map_err(Error::from)
    }

    #[inline]
    fn finish(self) -> Result<W> {
        self.writer.finish_document().map_err(Error::from)
    }

    #[inline]
    fn write_bool(&mut self, b: bool) -> Result<()> {
        self.writer.bool_value(b).map_err(Error::from)
    }

    #[inline]
    fn write_name(&mut self, name: &str) -> Result<()> {
        self.writer.name(name).map_err(Error::from)
    }

    #[inline]
    fn write_null(&mut self) -> Result<()> {
        self.writer.null_value().map_err(Error::from)
    }

    #[inline]
    fn write_number(&mut self, n: u32) -> Result<()> {
        self.writer.number_value(n).map_err(Error::from)
    }

    #[inline]
    fn write_string(&mut self, s: &str) -> Result<()> {
        self.writer.string_value(s).map_err(Error::from)
    }

    #[inline]
    fn write_string_streaming<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut dyn Write) -> std::io::Result<()>,
    {
        use struson::writer::StringValueWriter as _;

        let mut svw = self.writer.string_value_writer().map_err(Error::from)?;
        let inner_result = f(&mut svw);
        let finish_result = svw.finish_value().map_err(Error::from);
        inner_result.map_err(Error::from)?;
        finish_result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;
    use std::rc::Rc;

    struct ErrorWriter;

    impl Write for ErrorWriter {
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "test"))
        }
    }

    #[test]
    fn struson_backend_error_propagates_as_io_err() {
        let mut b = StrusonBackend::new(ErrorWriter);
        let result = b.begin_object();
        assert!(matches!(result, Err(Error::Io { .. })));
    }

    #[test]
    fn struson_backend_error_writer_flush_succeeds() {
        let mut w = ErrorWriter;
        assert!(w.flush().is_ok());
    }

    #[test]
    fn struson_backend_finish_returns_underlying_writer() {
        let mut b = StrusonBackend::new(Vec::new());
        assert!(b.begin_array().is_ok());
        assert!(b.end_array().is_ok());
        let result = b.finish();
        assert!(result.is_ok());
        let bytes = result.unwrap_or_default();
        assert!(bytes == b"[]");
    }

    #[test]
    fn struson_backend_writes_array_of_values() {
        let mut b = StrusonBackend::new(Vec::new());
        assert!(b.begin_array().is_ok());
        assert!(b.write_number(1).is_ok());
        assert!(b.write_bool(true).is_ok());
        assert!(b.write_null().is_ok());
        assert!(b.write_string("x").is_ok());
        assert!(b.end_array().is_ok());
        let result = b.finish();
        assert!(result.is_ok());
        let bytes = result.unwrap_or_default();
        assert!(bytes == br#"[1,true,null,"x"]"#);
    }

    #[test]
    fn struson_backend_writes_empty_object() {
        let mut b = StrusonBackend::new(Vec::new());
        assert!(b.begin_object().is_ok());
        assert!(b.end_object().is_ok());
        let result = b.finish();
        assert!(result.is_ok());
        let bytes = result.unwrap_or_default();
        assert!(bytes == b"{}");
    }

    #[test]
    fn struson_backend_writes_nested_structure() {
        let mut b = StrusonBackend::new(Vec::new());
        assert!(b.begin_object().is_ok());
        assert!(b.write_name("a").is_ok());
        assert!(b.begin_array().is_ok());
        assert!(b.write_number(1).is_ok());
        assert!(b.begin_object().is_ok());
        assert!(b.write_name("b").is_ok());
        assert!(b.write_bool(true).is_ok());
        assert!(b.end_object().is_ok());
        assert!(b.end_array().is_ok());
        assert!(b.end_object().is_ok());
        let result = b.finish();
        assert!(result.is_ok());
        let bytes = result.unwrap_or_default();
        assert!(bytes == br#"{"a":[1,{"b":true}]}"#);
    }

    #[test]
    fn struson_backend_writes_simple_key_value() {
        let mut b = StrusonBackend::new(Vec::new());
        assert!(b.begin_object().is_ok());
        assert!(b.write_name("k").is_ok());
        assert!(b.write_string("v").is_ok());
        assert!(b.end_object().is_ok());
        let result = b.finish();
        assert!(result.is_ok());
        let bytes = result.unwrap_or_default();
        assert!(bytes == br#"{"k":"v"}"#);
    }

    #[test]
    fn streaming_writes_simple_string() {
        let mut b = StrusonBackend::new(Vec::new());
        assert!(b.write_string_streaming(|w| w.write_all(b"hello")).is_ok());
        let result = b.finish();
        assert!(result.is_ok());
        let bytes = result.unwrap_or_default();
        assert!(bytes == br#""hello""#);
    }

    #[test]
    fn streaming_writes_base64_safe_chars() {
        let mut b = StrusonBackend::new(Vec::new());
        assert!(b
            .write_string_streaming(|w| w.write_all(b"data:image/png;base64,iVBORw=="))
            .is_ok());
        let result = b.finish();
        assert!(result.is_ok());
        let bytes = result.unwrap_or_default();
        assert!(bytes == br#""data:image/png;base64,iVBORw==""#);
    }

    #[test]
    fn streaming_writes_json_special_chars_are_escaped() {
        let mut b = StrusonBackend::new(Vec::new());
        assert!(b
            .write_string_streaming(|w| w.write_all(br#""quoted""#))
            .is_ok());
        let result = b.finish();
        assert!(result.is_ok());
        let bytes = result.unwrap_or_default();
        assert!(bytes == br#""\"quoted\"""#);
    }

    #[test]
    fn streaming_writes_multi_chunk() {
        let mut b = StrusonBackend::new(Vec::new());
        assert!(b
            .write_string_streaming(|w| {
                w.write_all(b"one")?;
                w.write_all(b"-")?;
                w.write_all(b"two")?;
                w.write_all(b"-")?;
                w.write_all(b"three")
            })
            .is_ok());
        let result = b.finish();
        assert!(result.is_ok());
        let bytes = result.unwrap_or_default();
        assert!(bytes == br#""one-two-three""#);
    }

    #[test]
    fn streaming_writes_error_in_closure_still_finishes_value() {
        let mut b = StrusonBackend::new(Vec::new());
        assert!(b.begin_object().is_ok());
        assert!(b.write_name("failed").is_ok());

        let result = b.write_string_streaming(|_| Err(std::io::Error::other("closure failed")));
        assert!(result.is_err());

        let after_key =
            std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| b.write_name("after")));
        #[allow(clippy::expect_used)]
        let write_result = after_key.expect("asserted catch_unwind returned Ok");
        assert!(write_result.is_ok());

        assert!(b.write_string("ok").is_ok());
        assert!(b.end_object().is_ok());
        let finish_result = b.finish();
        assert!(finish_result.is_ok());
        let bytes = finish_result.unwrap_or_default();
        assert!(bytes == br#"{"failed":"","after":"ok"}"#);
    }

    /// Tracks each write call to the inner writer.
    struct CallCountingWriter {
        write_calls: Rc<RefCell<Vec<usize>>>,
        buffer: Vec<u8>,
    }

    impl CallCountingWriter {
        fn new() -> Self {
            Self {
                write_calls: Rc::new(RefCell::new(Vec::new())),
                buffer: Vec::new(),
            }
        }

        fn write_calls(&self) -> Vec<usize> {
            self.write_calls.borrow().clone()
        }

        fn total_bytes(&self) -> usize {
            self.write_calls.borrow().iter().sum()
        }
    }

    impl Write for CallCountingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.write_calls.borrow_mut().push(buf.len());
            self.buffer.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn override_chunks_through_inner_writer() {
        // This test proves that write_string_streaming override is actually used,
        // not the default buffering impl. If the override is wired correctly,
        // the inner writer should see multiple separate write calls (streaming).
        // If buffering was used, we'd see one large write.

        let mut counter = CallCountingWriter::new();
        let mut b = StrusonBackend::new(&mut counter);

        // Write 1024 bytes in 4 separate chunks of 256 bytes each.
        // This is large enough that if buffering was used, we'd see ONE write of 1024 bytes.
        // If streaming is used (override is wired), we should see multiple smaller writes.
        let chunk = vec![b'x'; 256];
        assert!(b
            .write_string_streaming(|w| {
                w.write_all(&chunk)?;
                w.write_all(&chunk)?;
                w.write_all(&chunk)?;
                w.write_all(&chunk)
            })
            .is_ok());

        let write_calls = counter.write_calls();
        let total_bytes = counter.total_bytes();

        // Verify total bytes written is at least 1024 (the content we wrote).
        // The actual total may be slightly more due to JSON escaping/framing.
        assert!(
            total_bytes >= 1024,
            "Expected at least 1024 bytes written, got {total_bytes}"
        );

        // Verify that the largest single write is less than the total.
        // This proves streaming: if buffering was used, we'd see ONE write of ~1024 bytes.
        // If streaming is used, we see multiple smaller writes.
        let max_write = write_calls.iter().max().copied().unwrap_or(0);
        assert!(
             max_write < total_bytes,
             "Expected multiple writes (streaming), but got one large write: max={max_write}, total={total_bytes}"
         );

        // Verify we got multiple write calls (not just one).
        assert!(
            write_calls.len() > 1,
            "Expected multiple write calls (streaming), got {}",
            write_calls.len()
        );
    }
}
