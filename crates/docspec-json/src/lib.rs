#![forbid(unsafe_code)]

//! JSON writing primitives for docspec writers.
//!
//! Provides a generic [`JsonBackend`] trait, a fluent [`JsonEmitter`] API
//! with stack-based state validation, a default [`StrusonBackend`] adapter
//! for `struson::JsonStreamWriter`, and a high-level [`JsonWriter`] base
//! shared between event-driven JSON sinks.

pub mod backend;
pub mod emitter;
pub mod state;
pub mod struson_backend;
pub mod value;
pub mod writer;

pub use backend::{CapturingBackend, JsonBackend, Token};
pub use emitter::{JsonEmitter, KeyedEmitter};
pub use state::{Frame, KeyState};
pub use struson_backend::StrusonBackend;
pub use value::{Null, WriteVal};
pub use writer::JsonWriter;
