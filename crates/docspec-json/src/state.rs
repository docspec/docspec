//! State machine for tracking JSON emission context.

use docspec_core::{Error, Result};

/// A single frame on the emission stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// Inside an array.
    Array,
    /// Inside an object, awaiting either a key or a value.
    Object(KeyState),
    /// Root context (zero or one value allowed).
    Root,
}

/// Sub-state within an [`Frame::Object`] frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// The object is expecting the next key.
    ExpectingKey,
    /// A key was just written; awaiting its value.
    ExpectingValue,
}

/// Stack tracking the current JSON emission context.
///
/// Each push corresponds to opening an object or array; each pop corresponds
/// to closing one. The root frame represents the outermost document level.
pub struct StateStack {
    stack: Vec<Frame>,
}

impl StateStack {
    /// Return the current top frame, or `None` if the root value has been consumed.
    #[inline]
    #[must_use]
    pub fn current(&self) -> Option<&Frame> {
        self.stack.last()
    }

    /// Validate that a key is allowed. OK only when current frame is `Object(ExpectingKey)`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a key is not allowed at the current position.
    #[inline]
    pub fn expect_key_allowed(&self) -> Result<()> {
        match self.stack.last() {
            Some(Frame::Object(KeyState::ExpectingKey)) => Ok(()),
            Some(Frame::Object(KeyState::ExpectingValue)) => Err(Error::Json {
                message: "key not allowed: previous key has no value yet".to_string(),
                position: None,
            }),
            Some(Frame::Array) => Err(Error::Json {
                message: "key not allowed: inside an array".to_string(),
                position: None,
            }),
            Some(Frame::Root) => Err(Error::Json {
                message: "key not allowed: not inside an object".to_string(),
                position: None,
            }),
            None => Err(Error::Json {
                message: "key not allowed: no current frame".to_string(),
                position: None,
            }),
        }
    }

    /// Validate that a value is allowed. OK when: `Object(ExpectingValue)`, `Array`, or `Root`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if a value is not allowed at the current position.
    #[inline]
    pub fn expect_value_allowed(&self) -> Result<()> {
        match self.stack.last() {
            Some(Frame::Object(KeyState::ExpectingValue) | Frame::Array | Frame::Root) => Ok(()),
            Some(Frame::Object(KeyState::ExpectingKey)) => Err(Error::Json {
                message: "value not allowed: object expects a key first".to_string(),
                position: None,
            }),
            None => Err(Error::Json {
                message: "value not allowed: no current frame".to_string(),
                position: None,
            }),
        }
    }

    /// Returns `true` when all containers have been closed and the root value consumed.
    #[inline]
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.stack.is_empty()
    }

    /// Mark a key as written — transition Object from `ExpectingKey` to `ExpectingValue`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the current frame is not `Object(ExpectingKey)`.
    #[inline]
    pub fn mark_key_written(&mut self) -> Result<()> {
        match self.stack.last_mut() {
            Some(Frame::Object(ks)) if *ks == KeyState::ExpectingKey => {
                *ks = KeyState::ExpectingValue;
                Ok(())
            }
            Some(Frame::Object(_)) => Err(Error::Json {
                message: "key already written; expected a value next".to_string(),
                position: None,
            }),
            _ => Err(Error::Json {
                message: "cannot write key: not inside an object".to_string(),
                position: None,
            }),
        }
    }

    /// Mark a value as written.
    ///
    /// Transitions: `Object(ExpectingValue)` → `ExpectingKey`; `Root` → pop; `Array` → no-op.
    ///
    /// # Errors
    ///
    /// Returns `Err` if called in `Object(ExpectingKey)` state (value without key).
    #[inline]
    pub fn mark_value_written(&mut self) -> Result<()> {
        match self.stack.last_mut() {
            Some(Frame::Object(ks)) if *ks == KeyState::ExpectingValue => {
                *ks = KeyState::ExpectingKey;
                Ok(())
            }
            Some(Frame::Object(_)) => Err(Error::Json {
                message: "value without key: object expects a key".to_string(),
                position: None,
            }),
            Some(Frame::Array) => Ok(()),
            Some(Frame::Root) => {
                self.stack.pop();
                Ok(())
            }
            None => Err(Error::Json {
                message: "no current frame for value".to_string(),
                position: None,
            }),
        }
    }

    /// Create a new `StateStack` containing a single [`Frame::Root`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: vec![Frame::Root],
        }
    }

    /// Validate that the top frame is an Array without popping.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the current frame is not an Array.
    #[inline]
    pub fn peek_array(&self) -> Result<()> {
        match self.stack.last() {
            Some(Frame::Array) => Ok(()),
            Some(Frame::Object(_)) => Err(Error::Json {
                message: "cannot close array: current frame is an object".to_string(),
                position: None,
            }),
            _ => Err(Error::Json {
                message: "cannot close array: no open array".to_string(),
                position: None,
            }),
        }
    }

    /// Validate that the top frame is an Object in `ExpectingKey` state, without popping.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the current frame is not an Object, or if the Object is in
    /// `ExpectingValue` state (a key was written but its value hasn't been emitted).
    #[inline]
    pub fn peek_object(&self) -> Result<()> {
        match self.stack.last() {
            Some(Frame::Object(KeyState::ExpectingKey)) => Ok(()),
            Some(Frame::Object(KeyState::ExpectingValue)) => Err(Error::Json {
                message: "cannot close object: last key has no value".to_string(),
                position: None,
            }),
            Some(Frame::Array) => Err(Error::Json {
                message: "cannot close object: current frame is an array".to_string(),
                position: None,
            }),
            _ => Err(Error::Json {
                message: "cannot close object: no open object".to_string(),
                position: None,
            }),
        }
    }

    /// Pop the top frame. Returns `Err` if the stack is empty.
    ///
    /// # Errors
    ///
    /// Returns `Err` if there is no open container to close.
    #[inline]
    pub fn pop(&mut self) -> Result<Frame> {
        self.stack.pop().ok_or_else(|| Error::Json {
            message: "cannot close: no open container".to_string(),
            position: None,
        })
    }

    /// Pop the top frame, asserting it is an Array. Returns `Err` if it is not.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the current frame is not an Array.
    #[inline]
    pub fn pop_array(&mut self) -> Result<()> {
        match self.stack.last() {
            Some(Frame::Array) => {
                self.stack.pop();
                Ok(())
            }
            Some(Frame::Object(_)) => Err(Error::Json {
                message: "cannot close array: current frame is an object".to_string(),
                position: None,
            }),
            _ => Err(Error::Json {
                message: "cannot close array: no open array".to_string(),
                position: None,
            }),
        }
    }

    /// Pop the top frame, asserting it is an Object expecting a key. Returns `Err` if it is not.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the current frame is not an Object expecting a key.
    #[inline]
    pub fn pop_object(&mut self) -> Result<()> {
        match self.stack.last() {
            Some(Frame::Object(KeyState::ExpectingKey)) => {
                self.stack.pop();
                Ok(())
            }
            Some(Frame::Object(KeyState::ExpectingValue)) => Err(Error::Json {
                message: "cannot close object: last key has no value".to_string(),
                position: None,
            }),
            Some(Frame::Array) => Err(Error::Json {
                message: "cannot close object: current frame is an array".to_string(),
                position: None,
            }),
            _ => Err(Error::Json {
                message: "cannot close object: no open object".to_string(),
                position: None,
            }),
        }
    }

    /// Push a new Array frame.
    #[inline]
    pub fn push_array(&mut self) {
        self.stack.push(Frame::Array);
    }

    /// Push a new Object frame in `ExpectingKey` state.
    #[inline]
    pub fn push_object(&mut self) {
        self.stack.push(Frame::Object(KeyState::ExpectingKey));
    }
}

impl Default for StateStack {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_returns_root_frame() {
        let s = StateStack::new();
        assert_eq!(s.current(), Some(&Frame::Root));
    }

    #[test]
    fn push_pop_object_round_trip() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(matches!(
            s.current(),
            Some(Frame::Object(KeyState::ExpectingKey))
        ));
        assert!(s.pop_object().is_ok());
    }

    #[test]
    fn push_pop_array_round_trip() {
        let mut s = StateStack::new();
        s.push_array();
        assert_eq!(s.current(), Some(&Frame::Array));
        assert!(s.pop_array().is_ok());
    }

    #[test]
    fn nested_push_pop_lifo_order() {
        let mut s = StateStack::new();
        s.push_object();
        s.push_array();
        assert_eq!(s.current(), Some(&Frame::Array));
        assert!(s.pop_array().is_ok());
        assert!(matches!(s.current(), Some(Frame::Object(_))));
    }

    #[test]
    fn pop_at_root_returns_error() {
        let mut s = StateStack::new();
        s.stack.clear();
        assert!(s.pop().is_err());
    }

    #[test]
    fn peek_array_at_root_errors() {
        let s = StateStack::new();
        assert!(s.peek_array().is_err());
    }

    #[test]
    fn peek_array_when_top_is_array_succeeds() {
        let mut s = StateStack::new();
        s.push_array();
        assert!(s.peek_array().is_ok());
        assert_eq!(s.current(), Some(&Frame::Array));
    }

    #[test]
    fn peek_array_when_top_is_object_errors() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(s.peek_array().is_err());
    }

    #[test]
    fn peek_object_at_root_errors() {
        let s = StateStack::new();
        assert!(s.peek_object().is_err());
    }

    #[test]
    fn peek_object_when_top_is_array_errors() {
        let mut s = StateStack::new();
        s.push_array();
        assert!(s.peek_object().is_err());
    }

    #[test]
    fn peek_object_when_top_is_object_expecting_key_succeeds() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(s.peek_object().is_ok());
    }

    #[test]
    fn peek_object_when_top_is_object_expecting_value_errors() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(s.mark_key_written().is_ok());
        assert!(s.peek_object().is_err());
    }

    #[test]
    fn pop_object_when_top_is_object_succeeds() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(s.pop_object().is_ok());
    }

    #[test]
    fn pop_object_when_top_is_array_errors() {
        let mut s = StateStack::new();
        s.push_array();
        assert!(s.pop_object().is_err());
    }

    #[test]
    fn pop_object_at_root_errors() {
        let mut s = StateStack::new();
        assert!(s.pop_object().is_err());
    }

    #[test]
    fn pop_object_in_expecting_value_errors() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(s.mark_key_written().is_ok());
        assert!(s.pop_object().is_err());
    }

    #[test]
    fn pop_array_when_top_is_array_succeeds() {
        let mut s = StateStack::new();
        s.push_array();
        assert!(s.pop_array().is_ok());
    }

    #[test]
    fn pop_array_when_top_is_object_errors() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(s.pop_array().is_err());
    }

    #[test]
    fn pop_array_at_root_errors() {
        let mut s = StateStack::new();
        assert!(s.pop_array().is_err());
    }

    #[test]
    fn mark_key_written_in_object_expecting_key_transitions_to_expecting_value() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(s.mark_key_written().is_ok());
        assert!(matches!(
            s.current(),
            Some(Frame::Object(KeyState::ExpectingValue))
        ));
    }

    #[test]
    fn mark_key_written_in_object_expecting_value_errors() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(s.mark_key_written().is_ok());
        assert!(s.mark_key_written().is_err());
    }

    #[test]
    fn mark_key_written_in_array_errors() {
        let mut s = StateStack::new();
        s.push_array();
        assert!(s.mark_key_written().is_err());
    }

    #[test]
    fn mark_key_written_at_root_errors() {
        let mut s = StateStack::new();
        assert!(s.mark_key_written().is_err());
    }

    #[test]
    fn mark_value_written_in_object_expecting_value_transitions_back() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(s.mark_key_written().is_ok());
        assert!(s.mark_value_written().is_ok());
        assert!(matches!(
            s.current(),
            Some(Frame::Object(KeyState::ExpectingKey))
        ));
    }

    #[test]
    fn mark_value_written_in_object_expecting_key_errors() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(s.mark_value_written().is_err());
    }

    #[test]
    fn mark_value_written_without_frame_errors() {
        let mut s = StateStack::new();
        s.stack.clear();
        assert!(s.mark_value_written().is_err());
    }

    #[test]
    fn mark_value_written_in_array_no_transition() {
        let mut s = StateStack::new();
        s.push_array();
        assert!(s.mark_value_written().is_ok());
        assert_eq!(s.current(), Some(&Frame::Array));
    }

    #[test]
    fn mark_value_written_at_root_consumes_root() {
        let mut s = StateStack::new();
        assert!(s.mark_value_written().is_ok());
        assert!(s.is_finished());
    }

    #[test]
    fn expect_key_allowed_only_in_object_expecting_key() {
        let mut s = StateStack::new();
        assert!(s.expect_key_allowed().is_err());
        s.push_array();
        assert!(s.expect_key_allowed().is_err());
        assert!(s.pop_array().is_ok());
        s.push_object();
        assert!(s.expect_key_allowed().is_ok());
        assert!(s.mark_key_written().is_ok());
        assert!(s.expect_key_allowed().is_err());
    }

    #[test]
    fn expect_key_allowed_without_frame_errors() {
        let mut s = StateStack::new();
        s.stack.clear();
        assert!(s.expect_key_allowed().is_err());
    }

    #[test]
    fn expect_value_allowed_in_root_object_value_and_array_only() {
        let mut s = StateStack::new();
        assert!(s.expect_value_allowed().is_ok());
        s.push_object();
        assert!(s.expect_value_allowed().is_err());
        assert!(s.mark_key_written().is_ok());
        assert!(s.expect_value_allowed().is_ok());
        assert!(s.mark_value_written().is_ok());
        s.push_array();
        assert!(s.expect_value_allowed().is_ok());
    }

    #[test]
    fn expect_value_allowed_without_frame_errors() {
        let mut s = StateStack::new();
        s.stack.clear();
        assert!(s.expect_value_allowed().is_err());
    }

    #[test]
    fn is_finished_true_after_root_value_consumed() {
        let mut s = StateStack::new();
        assert!(s.mark_value_written().is_ok());
        assert!(s.is_finished());
    }

    #[test]
    fn is_finished_false_inside_open_container() {
        let mut s = StateStack::new();
        s.push_object();
        assert!(!s.is_finished());
    }

    #[test]
    fn state_stack_default_returns_root_frame() {
        let s = StateStack::default();
        assert_eq!(s.current(), Some(&Frame::Root));
    }
}
