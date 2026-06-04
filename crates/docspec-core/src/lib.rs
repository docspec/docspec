//! Core event types and traits for the `DocSpec` streaming document conversion library.
//!
//! `DocSpec` converts documents through a streaming event pipeline. This crate defines
//! the [`Event`] enum, supporting types, error types, and the [`EventSource`],
//! [`EventSink`], and [`AssetProvider`] traits that decouple readers from writers.
//!
//! # Quick Start
//!
//! Implement [`EventSource`] to produce events and [`EventSink`] to consume them.
//! Events represent document structure (headings, paragraphs, tables, text runs).
//! Readers and writers are fully decoupled through the event protocol.
//!
//! # Event Types
//!
//! The [`Event`] enum covers all document structures defined in the `DocSpec`
//! specification. See `EVENTS.md` for the authoritative variant list and semantics.

extern crate alloc;

mod depth;
mod error;
mod event;
mod pipeline;
mod stack;
mod traits;
mod types;

pub use depth::Depth;
pub use error::{Error, Position, Result};
pub use event::{Event, TextStyleKind};
pub use pipeline::pipe;
pub use stack::{block_kind_for_end, block_kind_for_start, BlockKind, StackTrackingSink};
pub use traits::{AssetProvider, EventSink, EventSource};
pub use types::{
    Author, Color, DocumentMeta, ImageSource, ListStyleType, TableHeaderScope, TextAlignment,
};
