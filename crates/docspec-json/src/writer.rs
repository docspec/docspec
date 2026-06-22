//! Streaming writer base for event-driven JSON sinks.
//!
//! Wraps a [`JsonEmitter`] over a [`StrusonBackend`] and adds paragraph-state
//! scaffolding shared between writers like `docspec-oxa-writer` and
//! `docspec-blocknote-writer`. Writers compose [`JsonWriter`] as a field
//! instead of re-spelling the `JsonEmitter::new(StrusonBackend::new(...))`
//! incantation in every constructor.
//!
//! Memory usage stays constant: every method is a thin forwarder onto the
//! emitter, no buffering is introduced.

use std::io::Write;

use docspec_core::Result;

use crate::{JsonEmitter, KeyedEmitter, StrusonBackend, WriteVal};

/// Streaming JSON writer base for [`docspec_core::EventSink`] implementations.
///
/// Owns the [`JsonEmitter`] + [`StrusonBackend`] composition that every JSON
/// writer in the workspace shares, exposes the emitter's structural API via
/// explicit forwarders, and tracks paragraph open/close state on behalf of
/// writers that emit a paragraph-shaped block.
///
/// # Streaming
///
/// All methods forward directly to the underlying emitter, which writes
/// tokens to the underlying [`Write`] as they arrive. No document is ever
/// buffered.
///
/// # Example
///
/// ```
/// use docspec_json::JsonWriter;
///
/// let mut buf = Vec::<u8>::new();
/// {
///     let mut w = JsonWriter::new(&mut buf);
///     w.open_object()?;
///     w.key("hello").value("world")?;
///     w.close_object()?;
///     w.finish()?;
/// }
/// assert_eq!(buf, br#"{"hello":"world"}"#);
/// # Ok::<(), docspec_core::Error>(())
/// ```
pub struct JsonWriter<W: Write> {
    /// Whether the writer has marked a paragraph-shaped block as currently
    /// open. Writers flip this on after emitting the block's opening tokens
    /// (via [`Self::set_paragraph_open`]) and rely on
    /// [`Self::close_paragraph`] to flip it off and emit the matching close.
    in_paragraph: bool,
    /// The underlying state-validated JSON emitter.
    json: JsonEmitter<StrusonBackend<W>>,
}

impl<W: Write> JsonWriter<W> {
    /// Create a new `JsonWriter` that streams JSON to `writer`.
    #[inline]
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self {
            in_paragraph: false,
            json: JsonEmitter::new(StrusonBackend::new(writer)),
        }
    }

    /// Finalize the JSON document and flush the underlying writer.
    ///
    /// Consumes `self` to prevent reuse after the stream ends, matching the
    /// shape of [`docspec_core::EventSink::finish`].
    ///
    /// # Errors
    ///
    /// Returns `Err` if the JSON stream is incomplete (open containers
    /// remain or no root value was written), or if the backend errors
    /// during flush.
    #[inline]
    pub fn finish(self) -> Result<()> {
        self.json.finish().map(|_| ())
    }

    /// Open an unkeyed array. Caller must later call [`Self::close_array`].
    ///
    /// # Errors
    ///
    /// See [`JsonEmitter::open_array`].
    #[inline]
    pub fn open_array(&mut self) -> Result<()> {
        self.json.open_array()
    }

    /// Close the current array frame.
    ///
    /// # Errors
    ///
    /// See [`JsonEmitter::close_array`].
    #[inline]
    pub fn close_array(&mut self) -> Result<()> {
        self.json.close_array()
    }

    /// Open an unkeyed object. Caller must later call [`Self::close_object`].
    ///
    /// # Errors
    ///
    /// See [`JsonEmitter::open_object`].
    #[inline]
    pub fn open_object(&mut self) -> Result<()> {
        self.json.open_object()
    }

    /// Close the current object frame.
    ///
    /// # Errors
    ///
    /// See [`JsonEmitter::close_object`].
    #[inline]
    pub fn close_object(&mut self) -> Result<()> {
        self.json.close_object()
    }

    /// Begin a keyed slot inside the current object.
    ///
    /// Returns a single-use [`KeyedEmitter`] that must be consumed by one of
    /// its `value`, `object`, `array`, `open_object`, `open_array`, or
    /// `string_value_streaming` methods.
    #[inline]
    pub fn key<'a>(&'a mut self, name: &'a str) -> KeyedEmitter<'a, StrusonBackend<W>> {
        self.json.key(name)
    }

    /// Write an unkeyed scalar value.
    ///
    /// # Errors
    ///
    /// See [`JsonEmitter::value`].
    #[inline]
    pub fn value<V: WriteVal>(&mut self, v: V) -> Result<()> {
        self.json.value(v)
    }

    /// Write an unkeyed object using a closure.
    ///
    /// The closure receives the inner [`JsonEmitter`] so callers can use
    /// every emitter API (including [`KeyedEmitter`] chains) inside the
    /// object's scope.
    ///
    /// # Errors
    ///
    /// See [`JsonEmitter::object`].
    #[inline]
    pub fn object<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut JsonEmitter<StrusonBackend<W>>) -> Result<()>,
    {
        self.json.object(f)
    }

    /// Write an unkeyed array using a closure.
    ///
    /// The closure receives the inner [`JsonEmitter`] so callers can use
    /// every emitter API inside the array's scope.
    ///
    /// # Errors
    ///
    /// See [`JsonEmitter::array`].
    #[inline]
    pub fn array<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut JsonEmitter<StrusonBackend<W>>) -> Result<()>,
    {
        self.json.array(f)
    }

    /// Returns whether a paragraph-shaped block is currently marked open.
    ///
    /// Pure state query. Writers call [`Self::set_paragraph_open`] when
    /// they emit a paragraph container's opening tokens, and
    /// [`Self::close_paragraph`] when they close it.
    #[inline]
    #[must_use]
    pub fn in_paragraph(&self) -> bool {
        self.in_paragraph
    }

    /// Mark the current paragraph as open.
    ///
    /// Call this after emitting the paragraph container's opening tokens
    /// (typically `{"type":"…","children":[` or similar). The matching
    /// close is emitted by [`Self::close_paragraph`].
    #[inline]
    pub fn set_paragraph_open(&mut self) {
        self.in_paragraph = true;
    }

    /// Close the currently open paragraph, if any.
    ///
    /// When [`Self::in_paragraph`] is `true`, emits `]}` (close the
    /// paragraph's inline-content array, then close the paragraph object)
    /// and clears the paragraph-open flag. When no paragraph is open this
    /// is a no-op and emits no bytes.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`JsonEmitter::close_array`] or
    /// [`JsonEmitter::close_object`] when the underlying stream is in a
    /// state that does not permit the matching close.
    #[inline]
    pub fn close_paragraph(&mut self) -> Result<()> {
        if !self.in_paragraph {
            return Ok(());
        }
        self.json.close_array()?;
        self.json.close_object()?;
        self.in_paragraph = false;
        Ok(())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Test-only helpers may panic on programmer error"
)]
mod tests {
    use super::*;
    use docspec_core::Error;

    /// Run a closure that produces JSON via a `JsonWriter`, returning the
    /// produced bytes as a `String`. Asserts that every step succeeded.
    fn capture<F>(write: F) -> String
    where
        F: for<'a> FnOnce(&mut JsonWriter<&'a mut Vec<u8>>) -> Result<()>,
    {
        let mut buf = Vec::<u8>::new();
        let mut w = JsonWriter::new(&mut buf);
        assert!(write(&mut w).is_ok(), "write closure should succeed");
        assert!(w.finish().is_ok(), "finish should succeed");
        String::from_utf8(buf).expect("valid utf-8")
    }

    mod construction {
        use super::*;

        #[test]
        fn new_does_not_emit_any_bytes() {
            let mut buf = Vec::<u8>::new();
            let mut w = JsonWriter::new(&mut buf);
            assert!(w.value("x").is_ok());
            assert!(w.finish().is_ok());
            assert_eq!(buf, br#""x""#);
        }
    }

    mod finish {
        use super::*;

        #[test]
        fn finish_after_root_scalar_succeeds() {
            let mut buf = Vec::<u8>::new();
            let mut w = JsonWriter::new(&mut buf);
            assert!(w.value("hello").is_ok());
            assert!(w.finish().is_ok());
            assert_eq!(buf, br#""hello""#);
        }

        #[test]
        fn finish_with_open_object_errors() {
            let mut buf = Vec::<u8>::new();
            let mut w = JsonWriter::new(&mut buf);
            assert!(w.open_object().is_ok());
            let result = w.finish();
            assert!(matches!(result, Err(Error::Json { .. })));
        }

        #[test]
        fn finish_with_open_array_errors() {
            let mut buf = Vec::<u8>::new();
            let mut w = JsonWriter::new(&mut buf);
            assert!(w.open_array().is_ok());
            let result = w.finish();
            assert!(matches!(result, Err(Error::Json { .. })));
        }

        #[test]
        fn finish_without_any_value_errors() {
            let mut buf = Vec::<u8>::new();
            let w = JsonWriter::new(&mut buf);
            assert!(matches!(w.finish(), Err(Error::Json { .. })));
        }
    }

    mod forwarders {
        use super::*;

        #[test]
        fn open_close_object_emits_empty_object() {
            let output = capture(|w| {
                w.open_object()?;
                w.close_object()
            });
            assert_eq!(output, "{}");
        }

        #[test]
        fn open_close_array_emits_empty_array() {
            let output = capture(|w| {
                w.open_array()?;
                w.close_array()
            });
            assert_eq!(output, "[]");
        }

        #[test]
        fn key_followed_by_value_emits_object_member() {
            let output = capture(|w| {
                w.open_object()?;
                w.key("a").value("1")?;
                w.close_object()
            });
            assert_eq!(output, r#"{"a":"1"}"#);
        }

        #[test]
        fn value_emits_root_scalar() {
            let output = capture(|w| w.value(true));
            assert_eq!(output, "true");
        }

        #[test]
        fn object_closure_emits_object_with_contents() {
            let output = capture(|w| {
                w.object(|j| {
                    j.key("type").value("Paragraph")?;
                    j.key("children").array(|_| Ok(()))
                })
            });
            assert_eq!(output, r#"{"type":"Paragraph","children":[]}"#);
        }

        #[test]
        fn array_closure_emits_array_with_contents() {
            let output = capture(|w| {
                w.array(|j| {
                    j.value("a")?;
                    j.value("b")
                })
            });
            assert_eq!(output, r#"["a","b"]"#);
        }

        #[test]
        fn close_array_when_top_is_object_errors() {
            let mut buf = Vec::<u8>::new();
            let mut w = JsonWriter::new(&mut buf);
            assert!(w.open_object().is_ok());
            assert!(matches!(w.close_array(), Err(Error::Json { .. })));
        }

        #[test]
        fn key_outside_object_errors() {
            let mut buf = Vec::<u8>::new();
            let mut w = JsonWriter::new(&mut buf);
            assert!(matches!(w.key("k").value("v"), Err(Error::Json { .. })));
        }
    }

    mod paragraph_state {
        use super::*;

        #[test]
        fn in_paragraph_defaults_to_false() {
            let buf = Vec::<u8>::new();
            let w = JsonWriter::new(buf);
            assert!(!w.in_paragraph());
        }

        #[test]
        fn set_paragraph_open_flips_flag() {
            let buf = Vec::<u8>::new();
            let mut w = JsonWriter::new(buf);
            w.set_paragraph_open();
            assert!(w.in_paragraph());
        }

        #[test]
        fn close_paragraph_when_closed_is_noop_and_emits_nothing() {
            let mut buf = Vec::<u8>::new();
            let mut w = JsonWriter::new(&mut buf);
            assert!(w.value("root").is_ok());
            assert!(w.close_paragraph().is_ok());
            assert!(!w.in_paragraph());
            assert!(w.finish().is_ok());
            assert_eq!(buf, br#""root""#);
        }

        #[test]
        fn close_paragraph_when_open_emits_close_array_then_close_object() {
            let output = capture(|w| {
                w.open_object()?;
                w.key("type").value("Paragraph")?;
                w.key("children").open_array()?;
                w.set_paragraph_open();
                w.close_paragraph()
            });
            assert_eq!(output, r#"{"type":"Paragraph","children":[]}"#);
        }

        #[test]
        fn close_paragraph_clears_in_paragraph_flag() {
            let mut buf = Vec::<u8>::new();
            let mut w = JsonWriter::new(&mut buf);
            assert!(w.open_array().is_ok());
            assert!(w.open_object().is_ok());
            assert!(w.key("children").open_array().is_ok());
            w.set_paragraph_open();
            assert!(w.in_paragraph());
            assert!(w.close_paragraph().is_ok());
            assert!(!w.in_paragraph());
            assert!(w.close_array().is_ok());
            assert!(w.finish().is_ok());
        }

        #[test]
        fn close_paragraph_called_twice_is_idempotent() {
            let output = capture(|w| {
                w.open_array()?;
                w.open_object()?;
                w.key("children").open_array()?;
                w.set_paragraph_open();
                w.close_paragraph()?;
                w.close_paragraph()?;
                w.close_array()
            });
            assert_eq!(output, r#"[{"children":[]}]"#);
        }
    }

    mod end_to_end {
        use super::*;

        #[test]
        fn full_oxa_shaped_document() {
            let output = capture(|w| {
                w.open_object()?;
                w.key("type").value("Document")?;
                w.key("children").open_array()?;

                w.open_object()?;
                w.key("type").value("Paragraph")?;
                w.key("children").open_array()?;
                w.set_paragraph_open();
                w.object(|j| {
                    j.key("type").value("Text")?;
                    j.key("value").value("Hello")
                })?;
                w.close_paragraph()?;

                w.close_array()?;
                w.close_object()
            });
            assert_eq!(
                output,
                r#"{"type":"Document","children":[{"type":"Paragraph","children":[{"type":"Text","value":"Hello"}]}]}"#
            );
        }

        #[test]
        fn full_blocknote_shaped_root_array() {
            let output = capture(|w| {
                w.open_array()?;
                w.object(|j| {
                    j.key("type").value("paragraph")?;
                    j.key("content").array(|c| {
                        c.object(|t| {
                            t.key("type").value("text")?;
                            t.key("text").value("Hi")
                        })
                    })
                })?;
                w.close_array()
            });
            assert_eq!(
                output,
                r#"[{"type":"paragraph","content":[{"type":"text","text":"Hi"}]}]"#
            );
        }
    }
}
