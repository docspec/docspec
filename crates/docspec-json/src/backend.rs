//! Backend trait for JSON token emission.

use docspec_core::Result;

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
    fn finish(self) -> Result<Self::Output>
    where
        Self: Sized;
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
    /// Write a JSON string value whose content is produced incrementally by the callback.
    ///
    /// The callback receives a `&mut dyn Write` positioned inside the JSON string slot.
    /// All bytes written to it are the raw string content (JSON-escaping is the backend's responsibility).
    ///
    /// **Always-close-string contract**: the JSON string is ALWAYS closed (closing `"` written) when
    /// this method returns, regardless of whether the callback returned `Ok` or `Err`. This is required
    /// to keep the underlying JSON state machine consistent.
    ///
    /// **Fail-fast contract**: on callback `Err`, the produced JSON is *structurally valid*
    /// (matching quotes) but *semantically incomplete* (truncated content). Callers MUST
    /// discard partial output on error — `DocSpec`'s "no partial conversions" philosophy applies.
    ///
    /// # Errors
    ///
    /// Returns any error produced by the callback or the underlying backend.
    fn write_string_streaming(
        &mut self,
        f: &mut dyn FnMut(&mut dyn std::io::Write) -> Result<()>,
    ) -> Result<()>;
}

/// A token captured by [`CapturingBackend`].
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Accumulates the callback's writes into a `Vec<u8>`, then emits a `Token::StringValue`.
    ///
    /// **Test-only accumulation**: this is intentionally NOT streaming (it buffers into a Vec).
    /// `CapturingBackend` is for verifying emitter state-machine behavior in unit tests only.
    /// Production code uses `StrusonBackend`, which streams without buffering.
    ///
    /// **Always-emit-token contract**: a `Token::StringValue` is ALWAYS pushed to the token list
    /// before returning, regardless of whether the callback returned `Ok` or `Err`. On `Err`,
    /// the token contains the partial content (whatever bytes were written before the error).
    /// The outer `Err` is then propagated. This mirrors struson's always-close-string behavior.
    #[inline]
    fn write_string_streaming(
        &mut self,
        f: &mut dyn FnMut(&mut dyn std::io::Write) -> Result<()>,
    ) -> Result<()> {
        let mut buf = Vec::new();
        let callback_result = f(&mut buf);
        // ALWAYS emit the token — even on Err (mirrors always-close-string contract)
        self.tokens.push(Token::StringValue(
            String::from_utf8_lossy(&buf).into_owned(),
        ));
        callback_result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docspec_core::Error;

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
        assert!(tokens == vec![Token::BeginObject, Token::EndObject]);
    }

    #[test]
    fn capturing_backend_streams_into_string_value() {
        let mut b = CapturingBackend::new();
        let result = b.write_string_streaming(&mut |w| {
            w.write_all(b"hello").map_err(|e| Error::Io { source: e })?;
            w.write_all(b" world")
                .map_err(|e| Error::Io { source: e })?;
            Ok(())
        });
        assert!(result.is_ok());
        assert_eq!(b.tokens(), &[Token::StringValue("hello world".to_string())]);
    }

    #[test]
    fn capturing_backend_streaming_error_emits_partial_token() {
        let mut b = CapturingBackend::new();
        let result = b.write_string_streaming(&mut |w| {
            w.write_all(b"partial")
                .map_err(|e| Error::Io { source: e })?;
            Err(Error::Other {
                message: "test error".to_string(),
            })
        });
        // Outer call returns the Err
        assert!(result.is_err());
        // BUT Token::StringValue was still emitted with the partial content
        assert_eq!(b.tokens(), &[Token::StringValue("partial".to_string())]);
    }
}
