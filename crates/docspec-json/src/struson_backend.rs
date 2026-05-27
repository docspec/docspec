//! [`JsonBackend`] adapter for `struson::JsonStreamWriter`.

use std::io::Write;

use struson::writer::{JsonStreamWriter, JsonWriter as _, StringValueWriter as _};

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
    fn write_string_streaming(
        &mut self,
        f: &mut dyn FnMut(&mut dyn std::io::Write) -> Result<()>,
    ) -> Result<()> {
        let mut string_writer = self.writer.string_value_writer().map_err(Error::from)?;
        let callback_result = f(&mut string_writer);
        let finish_result = string_writer.finish_value().map_err(Error::from);
        match (callback_result, finish_result) {
            (Err(err), _) | (Ok(()), Err(err)) => Err(err),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
