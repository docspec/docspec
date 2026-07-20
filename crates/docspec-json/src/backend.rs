//! Backend trait for JSON token emission.

use docspec_core::{Error, Result};

/// Low-level JSON token emitter. Implementations write JSON tokens to an
/// underlying sink. State validation is performed by [`crate::JsonEmitter`],
/// not by the backend.
pub trait JsonBackend {
    /// Type returned by [`JsonBackend::finish`].
    type Output;

    /// Begin a JSON array (writes `[`).
    ///
    /// # Errors
    ///
    /// Returns any error produced by the underlying backend.
    fn begin_array(&mut self) -> Result<()>;
    /// Begin a JSON object (writes `{`).
    ///
    /// # Errors
    ///
    /// Returns any error produced by the underlying backend.
    fn begin_object(&mut self) -> Result<()>;
    /// End the current JSON array (writes `]`).
    ///
    /// # Errors
    ///
    /// Returns any error produced by the underlying backend.
    fn end_array(&mut self) -> Result<()>;
    /// End the current JSON object (writes `}`).
    ///
    /// # Errors
    ///
    /// Returns any error produced by the underlying backend.
    fn end_object(&mut self) -> Result<()>;
    /// Finish writing and return the backend's output.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the underlying backend.
    fn finish(self) -> Result<Self::Output>;
    /// Write a boolean value.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the underlying backend.
    fn write_bool(&mut self, b: bool) -> Result<()>;
    /// Write an object key.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the underlying backend.
    fn write_name(&mut self, name: &str) -> Result<()>;
    /// Write a `null` value.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the underlying backend.
    fn write_null(&mut self) -> Result<()>;
    /// Write a `u32` numeric value.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the underlying backend.
    fn write_number(&mut self, n: u32) -> Result<()>;
    /// Write a string value.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the underlying backend.
    fn write_string(&mut self, s: &str) -> Result<()>;
    /// Write a string value by streaming bytes through a closure.
    ///
    /// The default implementation buffers the bytes into a `Vec<u8>`, converts
    /// to a `&str`, and delegates to [`write_string`](Self::write_string).
    ///
    /// # Errors
    ///
    /// Returns an error if the closure returns an I/O error, or if the buffered
    /// bytes are not valid UTF-8.
    #[allow(clippy::missing_inline_in_public_items)]
    fn write_string_streaming<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut dyn std::io::Write) -> std::io::Result<()>,
    {
        let mut buf: Vec<u8> = Vec::new();
        f(&mut buf).map_err(Error::from)?;
        let s = core::str::from_utf8(&buf).map_err(|e| Error::Other {
            message: format!("write_string_streaming produced invalid UTF-8: {e}"),
        })?;
        self.write_string(s)
    }
}

/// A token captured by [`CapturingBackend`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Token {
    /// Represents `[`.
    BeginArray,
    /// Represents `{`.
    BeginObject,
    /// A boolean value.
    BoolValue(bool),
    /// Represents `]`.
    EndArray,
    /// Represents `}`.
    EndObject,
    /// An object key.
    Name(String),
    /// A `null` value.
    NullValue,
    /// A `u32` numeric value.
    NumberValue(u32),
    /// A string value.
    StringValue(String),
}

/// Backend that records every operation as a [`Token`] without writing anywhere.
///
/// Useful for verifying [`crate::JsonEmitter`] behavior without depending on a
/// real JSON serializer.
#[derive(Debug, Default)]
pub struct CapturingBackend {
    tokens: Vec<Token>,
}

impl CapturingBackend {
    /// Create a new empty `CapturingBackend`.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrow the recorded tokens without consuming the backend.
    #[inline]
    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }
}

impl JsonBackend for CapturingBackend {
    type Output = Vec<Token>;

    #[inline]
    fn begin_array(&mut self) -> Result<()> {
        self.tokens.push(Token::BeginArray);
        Ok(())
    }

    #[inline]
    fn begin_object(&mut self) -> Result<()> {
        self.tokens.push(Token::BeginObject);
        Ok(())
    }

    #[inline]
    fn end_array(&mut self) -> Result<()> {
        self.tokens.push(Token::EndArray);
        Ok(())
    }

    #[inline]
    fn end_object(&mut self) -> Result<()> {
        self.tokens.push(Token::EndObject);
        Ok(())
    }

    #[inline]
    fn finish(self) -> Result<Vec<Token>> {
        Ok(self.tokens)
    }

    #[inline]
    fn write_bool(&mut self, b: bool) -> Result<()> {
        self.tokens.push(Token::BoolValue(b));
        Ok(())
    }

    #[inline]
    fn write_name(&mut self, name: &str) -> Result<()> {
        self.tokens.push(Token::Name(name.to_string()));
        Ok(())
    }

    #[inline]
    fn write_null(&mut self) -> Result<()> {
        self.tokens.push(Token::NullValue);
        Ok(())
    }

    #[inline]
    fn write_number(&mut self, n: u32) -> Result<()> {
        self.tokens.push(Token::NumberValue(n));
        Ok(())
    }

    #[inline]
    fn write_string(&mut self, s: &str) -> Result<()> {
        self.tokens.push(Token::StringValue(s.to_string()));
        Ok(())
    }

    #[inline]
    fn write_string_streaming<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut dyn std::io::Write) -> std::io::Result<()>,
    {
        let mut buf: Vec<u8> = Vec::new();
        f(&mut buf).map_err(Error::from)?;
        let s = core::str::from_utf8(&buf).map_err(|e| Error::Other {
            message: format!("write_string_streaming produced invalid UTF-8: {e}"),
        })?;
        self.tokens.push(Token::StringValue(s.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock backend that records `write_string` calls for testing the default impl.
    #[derive(Debug, Default)]
    struct MockBackend {
        write_string_calls: Vec<String>,
    }

    impl JsonBackend for MockBackend {
        type Output = ();

        fn begin_array(&mut self) -> Result<()> {
            Ok(())
        }

        fn begin_object(&mut self) -> Result<()> {
            Ok(())
        }

        fn end_array(&mut self) -> Result<()> {
            Ok(())
        }

        fn end_object(&mut self) -> Result<()> {
            Ok(())
        }

        fn finish(self) -> Result<()> {
            Ok(())
        }

        fn write_bool(&mut self, _b: bool) -> Result<()> {
            Ok(())
        }

        fn write_name(&mut self, _name: &str) -> Result<()> {
            Ok(())
        }

        fn write_null(&mut self) -> Result<()> {
            Ok(())
        }

        fn write_number(&mut self, _n: u32) -> Result<()> {
            Ok(())
        }

        fn write_string(&mut self, s: &str) -> Result<()> {
            self.write_string_calls.push(s.to_string());
            Ok(())
        }
    }

    #[test]
    fn default_impl_buffers_and_delegates_to_write_string() {
        let mut mock = MockBackend::default();
        let result = mock.write_string_streaming(|w| {
            w.write_all(b"hello world")?;
            Ok(())
        });
        assert!(result.is_ok());
        assert_eq!(mock.write_string_calls, vec!["hello world"]);
    }

    #[test]
    fn default_impl_errors_on_invalid_utf8() {
        let mut mock = MockBackend::default();
        let result = mock.write_string_streaming(|w| {
            w.write_all(&[0xFF, 0xFE, 0x00])?;
            Ok(())
        });
        assert!(result.is_err());
    }

    #[test]
    fn capturing_backend_starts_empty() {
        let b = CapturingBackend::new();
        assert!(b.tokens().is_empty());
    }

    #[test]
    fn capturing_backend_records_begin_object() {
        let mut b = CapturingBackend::new();
        assert!(b.begin_object().is_ok());
        assert_eq!(b.tokens(), &[Token::BeginObject]);
    }

    #[test]
    fn capturing_backend_records_all_token_types_in_order() {
        let mut b = CapturingBackend::new();
        assert!(b.begin_array().is_ok());
        assert!(b.begin_object().is_ok());
        assert!(b.write_name("k").is_ok());
        assert!(b.write_string("v").is_ok());
        assert!(b.write_bool(true).is_ok());
        assert!(b.write_number(42).is_ok());
        assert!(b.write_null().is_ok());
        assert!(b.end_object().is_ok());
        assert!(b.end_array().is_ok());
        assert_eq!(
            b.tokens(),
            &[
                Token::BeginArray,
                Token::BeginObject,
                Token::Name("k".to_string()),
                Token::StringValue("v".to_string()),
                Token::BoolValue(true),
                Token::NumberValue(42),
                Token::NullValue,
                Token::EndObject,
                Token::EndArray,
            ]
        );
    }

    #[test]
    fn capturing_backend_finish_returns_recorded_tokens() {
        let mut b = CapturingBackend::new();
        assert!(b.begin_object().is_ok());
        assert!(b.end_object().is_ok());
        let result = b.finish();
        assert!(result.is_ok());
        let tokens = result.unwrap_or_default();
        assert_eq!(tokens, vec![Token::BeginObject, Token::EndObject]);
    }

    #[test]
    fn capturing_streaming_pushes_single_token() {
        let mut b = CapturingBackend::new();
        let result = b.write_string_streaming(|w| w.write_all(b"hello"));
        assert!(result.is_ok());
        assert_eq!(b.tokens(), &[Token::StringValue("hello".to_string())]);
    }

    #[test]
    fn capturing_streaming_multi_chunk_concatenates() {
        let mut b = CapturingBackend::new();
        let result = b.write_string_streaming(|w| {
            w.write_all(b"hello")?;
            w.write_all(b" ")?;
            w.write_all(b"world")?;
            Ok(())
        });
        assert!(result.is_ok());
        assert_eq!(b.tokens(), &[Token::StringValue("hello world".to_string())]);
    }

    #[test]
    fn capturing_streaming_error_does_not_push_token() {
        let mut b = CapturingBackend::new();
        let result = b.write_string_streaming(|_w| Err(std::io::Error::other("test error")));
        assert!(result.is_err());
        assert!(b.tokens().is_empty());
    }
}
