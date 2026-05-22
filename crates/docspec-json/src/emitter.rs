//! Fluent JSON emitter with stack-based state validation.

use crate::{backend::JsonBackend, state::StateStack, value::WriteVal};
use docspec_core::{Error, Result};

/// Fluent JSON emitter generic over a [`JsonBackend`].
///
/// Drives the backend through valid JSON shapes only — invalid sequences
/// (e.g. `key` outside an object, two values for one key) return errors
/// before reaching the backend.
///
/// Provides two complementary APIs sharing the same state machine:
/// - **Closure form** — `object(f)`, `array(f)`, `value(v)` plus keyed via [`KeyedEmitter`].
/// - **Streaming form** — `open_object()`, `close_object()`, `open_array()`, `close_array()`.
pub struct JsonEmitter<B: JsonBackend> {
    backend: B,
    stack: StateStack,
}

impl<B: JsonBackend> JsonEmitter<B> {
    /// Write an unkeyed array using a closure.
    ///
    /// Best-effort scope guard: if the closure returns `Err`, [`Self::close_array`]
    /// is still attempted so that the backend sees a matching `end_array` and the
    /// emitter state machine is left in a consistent shape. The closure's error
    /// takes precedence over any error produced by the close.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a value is not allowed here, or if the closure or backend errors.
    #[inline]
    pub fn array<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        self.open_array()?;
        let inner = f(self);
        let close = self.close_array();
        inner.and(close)
    }

    /// Close the current array frame. Errors if the current frame is not an array.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the current frame is not an array, or if the backend errors.
    #[inline]
    pub fn close_array(&mut self) -> Result<()> {
        self.stack.peek_array()?;
        self.backend.end_array()?;
        self.stack.pop_array()?;
        self.stack.mark_value_written()
    }

    /// Close the current object frame. Errors if the current frame is not an object.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the current frame is not an object, or if the backend errors.
    #[inline]
    pub fn close_object(&mut self) -> Result<()> {
        self.stack.peek_object()?;
        self.backend.end_object()?;
        self.stack.pop_object()?;
        self.stack.mark_value_written()
    }

    /// Finish emission and return the backend's output.
    ///
    /// # Errors
    ///
    /// Returns `Err` if emission is incomplete, or if the backend errors during finish.
    #[inline]
    pub fn finish(self) -> Result<B::Output> {
        if !self.stack.is_finished() {
            return Err(Error::Json {
                message: "cannot finish: open containers remain or no root value written"
                    .to_string(),
                position: None,
            });
        }
        self.backend.finish()
    }

    /// Begin a keyed slot. Returns a single-use [`KeyedEmitter`] that must be
    /// consumed by `.object()`, `.array()`, `.value()`, `.open_object()`, or `.open_array()`.
    #[inline]
    pub fn key<'a>(&'a mut self, name: &'a str) -> KeyedEmitter<'a, B> {
        KeyedEmitter {
            emitter: self,
            name,
        }
    }

    /// Create a new emitter wrapping `backend`.
    #[inline]
    #[must_use]
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            stack: StateStack::new(),
        }
    }

    /// Write an unkeyed object using a closure.
    ///
    /// Best-effort scope guard: if the closure returns `Err`, [`Self::close_object`]
    /// is still attempted so that the backend sees a matching `end_object` and the
    /// emitter state machine is left in a consistent shape. The closure's error
    /// takes precedence over any error produced by the close.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a value is not allowed here, or if the closure or backend errors.
    #[inline]
    pub fn object<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        self.open_object()?;
        let inner = f(self);
        let close = self.close_object();
        inner.and(close)
    }

    /// Open an unkeyed array. Caller must later call [`JsonEmitter::close_array`].
    ///
    /// # Errors
    ///
    /// Returns `Err` if a value is not allowed here, or if the backend errors.
    #[inline]
    pub fn open_array(&mut self) -> Result<()> {
        self.stack.expect_value_allowed()?;
        self.backend.begin_array()?;
        self.stack.push_array();
        Ok(())
    }

    /// Open an unkeyed object. Caller must later call [`JsonEmitter::close_object`].
    ///
    /// # Errors
    ///
    /// Returns `Err` if a value is not allowed here, or if the backend errors.
    #[inline]
    pub fn open_object(&mut self) -> Result<()> {
        self.stack.expect_value_allowed()?;
        self.backend.begin_object()?;
        self.stack.push_object();
        Ok(())
    }

    /// Write an unkeyed scalar value.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a value is not allowed here, or if the backend errors.
    #[inline]
    pub fn value<V: WriteVal>(&mut self, v: V) -> Result<()> {
        self.stack.expect_value_allowed()?;
        v.write_to(&mut self.backend)?;
        self.stack.mark_value_written()
    }

    /// Write a key name and transition the object state machine.
    fn write_key(&mut self, name: &str) -> Result<()> {
        self.stack.expect_key_allowed()?;
        self.backend.write_name(name)?;
        self.stack.mark_key_written()
    }
}

/// Single-use handle returned by [`JsonEmitter::key`].
///
/// Consumed by [`KeyedEmitter::object`], [`KeyedEmitter::array`],
/// [`KeyedEmitter::value`], [`KeyedEmitter::open_object`], or
/// [`KeyedEmitter::open_array`].
#[must_use = "KeyedEmitter must be consumed by .object(), .array(), .value(), .open_object(), or .open_array()"]
pub struct KeyedEmitter<'a, B: JsonBackend> {
    emitter: &'a mut JsonEmitter<B>,
    name: &'a str,
}

impl<B: JsonBackend> KeyedEmitter<'_, B> {
    /// Consume the key and write an array value via closure.
    ///
    /// Best-effort scope guard: if the closure returns `Err`, the array close is
    /// still attempted; the closure's error takes precedence.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a key is not allowed here, or if the closure or backend errors.
    #[inline]
    pub fn array<F>(self, f: F) -> Result<()>
    where
        F: FnOnce(&mut JsonEmitter<B>) -> Result<()>,
    {
        let emitter = self.emitter;
        emitter.write_key(self.name)?;
        emitter.open_array()?;
        let inner = f(emitter);
        let close = emitter.close_array();
        inner.and(close)
    }

    /// Consume the key and write an object value via closure.
    ///
    /// Best-effort scope guard: if the closure returns `Err`, the object close is
    /// still attempted; the closure's error takes precedence.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a key is not allowed here, or if the closure or backend errors.
    #[inline]
    pub fn object<F>(self, f: F) -> Result<()>
    where
        F: FnOnce(&mut JsonEmitter<B>) -> Result<()>,
    {
        let emitter = self.emitter;
        emitter.write_key(self.name)?;
        emitter.open_object()?;
        let inner = f(emitter);
        let close = emitter.close_object();
        inner.and(close)
    }

    /// Consume the key and open an array. Caller must later call
    /// [`JsonEmitter::close_array`] on the underlying emitter.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a key is not allowed here, or if the backend errors.
    #[inline]
    pub fn open_array(self) -> Result<()> {
        self.emitter.write_key(self.name)?;
        self.emitter.open_array()
    }

    /// Consume the key and open an object. Caller must later call
    /// [`JsonEmitter::close_object`] on the underlying emitter.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a key is not allowed here, or if the backend errors.
    #[inline]
    pub fn open_object(self) -> Result<()> {
        self.emitter.write_key(self.name)?;
        self.emitter.open_object()
    }

    /// Consume the key and write a scalar value.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a key is not allowed here, or if the backend errors.
    #[inline]
    pub fn value<V: WriteVal>(self, v: V) -> Result<()> {
        let emitter = self.emitter;
        let name = self.name;
        emitter.write_key(name)?;
        v.write_to(&mut emitter.backend)?;
        emitter.stack.mark_value_written()
    }
}

#[cfg(test)]
mod tests {
    mod errors {
        use super::*;

        #[test]
        fn array_in_object_without_key_errors() {
            let mut e = make();
            assert!(e.object(|j| j.array(|_| Ok(()))).is_err());
        }

        #[test]
        fn close_array_when_top_is_object_errors() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            assert!(e.close_array().is_err());
        }

        #[test]
        fn close_object_at_root_errors() {
            let mut e = make();
            assert!(e.close_object().is_err());
        }

        #[test]
        fn close_object_when_top_is_array_errors() {
            let mut e = make();
            assert!(e.open_array().is_ok());
            assert!(e.close_object().is_err());
        }

        #[test]
        fn closure_error_in_array_still_closes_array() {
            let mut e = make();
            let inner = e.array(|j| {
                j.value("first")?;
                Err(docspec_core::Error::Other {
                    message: "inner".to_string(),
                })
            });
            assert!(inner.is_err());
            let tokens = finish_tokens(e);
            assert_eq!(
                tokens,
                vec![
                    Token::BeginArray,
                    Token::StringValue("first".to_string()),
                    Token::EndArray,
                ]
            );
        }

        #[test]
        fn closure_error_in_object_still_closes_object() {
            let mut e = make();
            let inner = e.object(|j| {
                j.key("a").value("1")?;
                Err(docspec_core::Error::Other {
                    message: "inner".to_string(),
                })
            });
            assert!(inner.is_err());
            let tokens = finish_tokens(e);
            assert_eq!(
                tokens,
                vec![
                    Token::BeginObject,
                    Token::Name("a".to_string()),
                    Token::StringValue("1".to_string()),
                    Token::EndObject,
                ]
            );
        }

        #[test]
        fn closure_error_propagates() {
            let mut e = make();
            let result = e.object(|_| {
                Err(docspec_core::Error::Other {
                    message: "inner error".to_string(),
                })
            });
            assert!(result.is_err());
        }

        #[test]
        fn dropped_key_does_not_corrupt_state() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            let k = e.key("x");
            drop(k);
            assert!(e.key("y").value("z").is_ok());
            assert!(e.close_object().is_ok());
        }

        #[test]
        fn finish_after_root_value_succeeds() {
            let mut e = make();
            assert!(e.value("x").is_ok());
            assert!(e.finish().is_ok());
        }

        #[test]
        fn finish_with_open_array_errors() {
            let mut e = make();
            assert!(e.open_array().is_ok());
            assert!(e.finish().is_err());
        }

        #[test]
        fn finish_with_open_object_errors() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            assert!(e.finish().is_err());
        }

        #[test]
        fn finish_without_any_value_errors() {
            let e = make();
            assert!(e.finish().is_err());
        }

        #[test]
        fn key_inside_array_errors() {
            let mut e = make();
            assert!(e.array(|j| j.key("x").value("1")).is_err());
        }

        #[test]
        fn key_outside_object_errors() {
            let mut e = make();
            assert!(e.key("x").value("1").is_err());
        }

        #[test]
        fn keyed_array_closure_error_still_closes_array() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            let inner = e.key("items").array(|j| {
                j.value("first")?;
                Err(docspec_core::Error::Other {
                    message: "inner".to_string(),
                })
            });
            assert!(inner.is_err());
            assert!(e.close_object().is_ok());
            let tokens = finish_tokens(e);
            assert_eq!(
                tokens,
                vec![
                    Token::BeginObject,
                    Token::Name("items".to_string()),
                    Token::BeginArray,
                    Token::StringValue("first".to_string()),
                    Token::EndArray,
                    Token::EndObject,
                ]
            );
        }

        #[test]
        fn keyed_object_closure_error_still_closes_object() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            let inner = e.key("inner").object(|j| {
                j.key("a").value("1")?;
                Err(docspec_core::Error::Other {
                    message: "inner".to_string(),
                })
            });
            assert!(inner.is_err());
            assert!(e.close_object().is_ok());
            let tokens = finish_tokens(e);
            assert_eq!(
                tokens,
                vec![
                    Token::BeginObject,
                    Token::Name("inner".to_string()),
                    Token::BeginObject,
                    Token::Name("a".to_string()),
                    Token::StringValue("1".to_string()),
                    Token::EndObject,
                    Token::EndObject,
                ]
            );
        }

        #[test]
        fn object_in_object_without_key_errors() {
            let mut e = make();
            assert!(e.object(|j| j.object(|_| Ok(()))).is_err());
        }

        #[test]
        fn open_object_in_object_expecting_value_errors() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            assert!(e.key("k").open_object().is_ok());
            assert!(e.open_object().is_err());
        }

        #[test]
        fn state_validation_uses_json_error_variant() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            let err = e.value("x");
            assert!(matches!(err, Err(docspec_core::Error::Json { .. })));
        }

        #[test]
        fn value_at_root_after_root_value_already_written_errors() {
            let mut e = make();
            assert!(e.value("first").is_ok());
            assert!(e.value("second").is_err());
        }

        #[test]
        fn value_in_object_without_key_errors() {
            let mut e = make();
            assert!(e.object(|j| j.value("x")).is_err());
        }
    }

    mod happy_path {
        use super::*;

        #[test]
        fn array_of_objects() {
            let mut e = make();
            assert!(e
                .array(|j| { j.object(|j2| j2.key("x").value("a")) })
                .is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginArray,
                    Token::BeginObject,
                    Token::Name("x".to_string()),
                    Token::StringValue("a".to_string()),
                    Token::EndObject,
                    Token::EndArray,
                ]
            );
        }

        #[test]
        fn array_of_scalars() {
            let mut e = make();
            assert!(e
                .array(|j| {
                    j.value("a")?;
                    j.value(true)?;
                    j.value(Null)
                })
                .is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginArray,
                    Token::StringValue("a".to_string()),
                    Token::BoolValue(true),
                    Token::NullValue,
                    Token::EndArray,
                ]
            );
        }

        #[test]
        fn finish_returns_backend_output_tokens() {
            let mut e = make();
            assert!(e.value("hi").is_ok());
            let t = finish_tokens(e);
            assert_eq!(t, vec![Token::StringValue("hi".to_string())]);
        }

        #[test]
        fn mixed_nested_structure_object_array_object() {
            let mut e = make();
            assert!(e
                .object(|j| {
                    j.key("a")
                        .array(|j2| j2.object(|j3| j3.key("b").value(true)))
                })
                .is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginObject,
                    Token::Name("a".to_string()),
                    Token::BeginArray,
                    Token::BeginObject,
                    Token::Name("b".to_string()),
                    Token::BoolValue(true),
                    Token::EndObject,
                    Token::EndArray,
                    Token::EndObject,
                ]
            );
        }

        #[test]
        fn nested_object_in_object() {
            let mut e = make();
            assert!(e
                .object(|j| { j.key("inner").object(|j2| j2.key("k").value("v")) })
                .is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginObject,
                    Token::Name("inner".to_string()),
                    Token::BeginObject,
                    Token::Name("k".to_string()),
                    Token::StringValue("v".to_string()),
                    Token::EndObject,
                    Token::EndObject,
                ]
            );
        }

        #[test]
        fn object_with_multiple_keys_in_order() {
            let mut e = make();
            assert!(e
                .object(|j| {
                    j.key("a").value("1")?;
                    j.key("b").value(true)?;
                    j.key("c").value(Null)
                })
                .is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginObject,
                    Token::Name("a".to_string()),
                    Token::StringValue("1".to_string()),
                    Token::Name("b".to_string()),
                    Token::BoolValue(true),
                    Token::Name("c".to_string()),
                    Token::NullValue,
                    Token::EndObject,
                ]
            );
        }

        #[test]
        fn object_with_single_key_string_value() {
            let mut e = make();
            assert!(e.object(|j| j.key("k").value("v")).is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginObject,
                    Token::Name("k".to_string()),
                    Token::StringValue("v".to_string()),
                    Token::EndObject,
                ]
            );
        }

        #[test]
        fn root_array_empty() {
            let mut e = make();
            assert!(e.array(|_| Ok(())).is_ok());
            assert_eq!(finish_tokens(e), vec![Token::BeginArray, Token::EndArray]);
        }

        #[test]
        fn root_object_empty() {
            let mut e = make();
            assert!(e.object(|_| Ok(())).is_ok());
            assert_eq!(finish_tokens(e), vec![Token::BeginObject, Token::EndObject]);
        }

        #[test]
        fn root_scalar_null() {
            let mut e = make();
            assert!(e.value(Null).is_ok());
            assert_eq!(finish_tokens(e), vec![Token::NullValue]);
        }

        #[test]
        fn root_scalar_string() {
            let mut e = make();
            assert!(e.value("hello").is_ok());
            assert_eq!(
                finish_tokens(e),
                vec![Token::StringValue("hello".to_string())]
            );
        }
    }

    mod streaming {
        use super::*;

        #[test]
        fn keyed_open_array_then_close() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            assert!(e.key("content").open_array().is_ok());
            assert!(e.value("x").is_ok());
            assert!(e.close_array().is_ok());
            assert!(e.close_object().is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginObject,
                    Token::Name("content".to_string()),
                    Token::BeginArray,
                    Token::StringValue("x".to_string()),
                    Token::EndArray,
                    Token::EndObject,
                ]
            );
        }

        #[test]
        fn keyed_open_object_then_close() {
            let mut e = make();
            assert!(e.open_array().is_ok());
            assert!(e.open_object().is_ok());
            assert!(e.key("type").value("h").is_ok());
            assert!(e.close_object().is_ok());
            assert!(e.close_array().is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginArray,
                    Token::BeginObject,
                    Token::Name("type".to_string()),
                    Token::StringValue("h".to_string()),
                    Token::EndObject,
                    Token::EndArray,
                ]
            );
        }

        #[test]
        fn keyed_open_then_unkeyed_open_in_array() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            assert!(e.key("c").open_array().is_ok());
            assert!(e.open_object().is_ok());
            assert!(e.close_object().is_ok());
            assert!(e.close_array().is_ok());
            assert!(e.close_object().is_ok());
            assert!(!finish_tokens(e).is_empty());
        }

        #[test]
        fn mixed_closure_and_streaming() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            assert!(e.key("k1").value("a").is_ok());
            assert!(e.key("k2").array(|j| j.value(true)).is_ok());
            assert!(e.close_object().is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginObject,
                    Token::Name("k1".to_string()),
                    Token::StringValue("a".to_string()),
                    Token::Name("k2".to_string()),
                    Token::BeginArray,
                    Token::BoolValue(true),
                    Token::EndArray,
                    Token::EndObject,
                ]
            );
        }

        #[test]
        fn nested_streaming_objects() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            assert!(e.key("a").open_object().is_ok());
            assert!(e.key("b").open_object().is_ok());
            assert!(e.close_object().is_ok());
            assert!(e.close_object().is_ok());
            assert!(e.close_object().is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginObject,
                    Token::Name("a".to_string()),
                    Token::BeginObject,
                    Token::Name("b".to_string()),
                    Token::BeginObject,
                    Token::EndObject,
                    Token::EndObject,
                    Token::EndObject,
                ]
            );
        }

        #[test]
        fn open_close_array_empty() {
            let mut e = make();
            assert!(e.open_array().is_ok());
            assert!(e.close_array().is_ok());
            assert_eq!(finish_tokens(e), vec![Token::BeginArray, Token::EndArray]);
        }

        #[test]
        fn open_close_object_empty() {
            let mut e = make();
            assert!(e.open_object().is_ok());
            assert!(e.close_object().is_ok());
            assert_eq!(finish_tokens(e), vec![Token::BeginObject, Token::EndObject]);
        }

        #[test]
        fn streaming_emits_blocknote_block_shape() {
            let mut e = make();
            assert!(e.open_array().is_ok());
            assert!(e.open_object().is_ok());
            assert!(e.key("type").value("heading").is_ok());
            assert!(e.key("content").open_array().is_ok());
            assert!(e.close_array().is_ok());
            assert!(e.close_object().is_ok());
            assert!(e.close_array().is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginArray,
                    Token::BeginObject,
                    Token::Name("type".to_string()),
                    Token::StringValue("heading".to_string()),
                    Token::Name("content".to_string()),
                    Token::BeginArray,
                    Token::EndArray,
                    Token::EndObject,
                    Token::EndArray,
                ]
            );
        }

        #[test]
        fn streaming_root_array_with_two_objects_via_open_close() {
            let mut e = make();
            assert!(e.open_array().is_ok());
            assert!(e.open_object().is_ok());
            assert!(e.key("a").value("1").is_ok());
            assert!(e.close_object().is_ok());
            assert!(e.open_object().is_ok());
            assert!(e.key("b").value("2").is_ok());
            assert!(e.close_object().is_ok());
            assert!(e.close_array().is_ok());
            let t = finish_tokens(e);
            assert_eq!(
                t,
                vec![
                    Token::BeginArray,
                    Token::BeginObject,
                    Token::Name("a".to_string()),
                    Token::StringValue("1".to_string()),
                    Token::EndObject,
                    Token::BeginObject,
                    Token::Name("b".to_string()),
                    Token::StringValue("2".to_string()),
                    Token::EndObject,
                    Token::EndArray,
                ]
            );
        }
    }

    use super::*;
    use crate::backend::{CapturingBackend, Token};
    use crate::value::Null;

    fn finish_tokens(e: JsonEmitter<CapturingBackend>) -> Vec<Token> {
        let result = e.finish();
        assert!(result.is_ok(), "finish should succeed");
        result.unwrap_or_default()
    }

    fn make() -> JsonEmitter<CapturingBackend> {
        JsonEmitter::new(CapturingBackend::new())
    }
}
