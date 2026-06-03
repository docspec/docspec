//! # `DocSpec`
//!
//! A streaming document conversion library. Documents are streams of typed events
//! flowing from [`EventSource`] readers to [`EventSink`] writers. Readers and writers
//! are fully decoupled — any reader can connect to any writer.
//!
//! This crate is a thin convenience facade over the modular `DocSpec` workspace.
//! It re-exports the core event types and traits, plus the optional format readers
//! and writers that you opt into through feature flags.
//!
//! # Feature Flags
//!
//! ## Readers
//!
//! | Feature    | Format                                              | Re-exports                  |
//! |------------|-----------------------------------------------------|-----------------------------|
//! | `markdown` | Markdown (`CommonMark` + GFM tables/strikethrough)  | [`readers::MarkdownReader`] |
//! | `html`     | HTML (paragraphs only)                              | [`readers::HtmlReader`]     |
//!
//! ## Writers
//!
//! | Feature     | Format           | Re-exports                    |
//! |-------------|------------------|-------------------------------|
//! | `blocknote` | `BlockNote` JSON | [`writers::BlockNoteWriter`]  |
//!
//! ## Primitives
//!
//! | Feature | Re-exports                                                            |
//! |---------|-----------------------------------------------------------------------|
//! | `json`  | [`json`] — streaming JSON emission primitives for custom writers      |
//!
//! ## Convenience
//!
//! | Feature       | Enables                                                |
//! |---------------|--------------------------------------------------------|
//! | `all-readers` | All reader features                                    |
//! | `all-writers` | All writer features                                    |
//! | `full`        | Everything (`all-readers` + `all-writers` + `json`)    |
//!
//! # Choosing the Right Dependency
//!
//! Use this `docspec` crate when you want a single convenient entry point and
//! you're happy to opt into formats via features. For the smallest possible
//! dependency footprint, depend directly on the individual sub-crates
//! (`docspec-core`, `docspec-markdown-reader`, etc.) instead.
//!
//! # Quick Start
//!
//! Add `docspec` to your `Cargo.toml` with the features you need:
//!
//! ```toml
//! [dependencies]
//! docspec = { version = "0.5", features = ["markdown", "blocknote"] }
//! ```
//!
//! Convert Markdown to `BlockNote` JSON:
//!
//! ```no_run
//! # #[cfg(all(feature = "markdown", feature = "blocknote"))]
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use docspec::readers::MarkdownReader;
//! use docspec::writers::BlockNoteWriter;
//! use docspec::{EventSink, EventSource, StackTrackingSink};
//!
//! let markdown = "# Hello\n\nWorld";
//! let mut reader = MarkdownReader::new(markdown);
//! let mut buf = Vec::<u8>::new();
//! let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
//!
//! while let Some(event) = reader.next_event()? {
//!     writer.handle_event(event)?;
//! }
//! writer.finish()?;
//!
//! let _json = String::from_utf8(buf)?;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

/// Commonly used items, brought into scope with a single import.
///
/// `use docspec::prelude::*;` imports the types and traits used in most `DocSpec` code.
pub mod prelude {
    pub use docspec_core::{Event, EventSink, EventSource, Result};
}

/// Document readers (event sources).
///
/// Each reader is gated by a feature flag. See the crate-level documentation for the
/// full list of supported formats and corresponding feature flags.
pub mod readers {
    /// Streaming Markdown reader. Available when the `markdown` feature is enabled.
    #[cfg(feature = "markdown")]
    #[cfg_attr(docsrs, doc(cfg(feature = "markdown")))]
    pub use docspec_markdown_reader::MarkdownReader;

    /// Streaming HTML reader. Available when the `html` feature is enabled.
    /// Note: currently parses only `<p>` elements and text within them.
    #[cfg(feature = "html")]
    #[cfg_attr(docsrs, doc(cfg(feature = "html")))]
    pub use docspec_html_reader::HtmlReader;
}

/// Document writers (event sinks).
///
/// Each writer is gated by a feature flag. See the crate-level documentation for the
/// full list of supported formats and corresponding feature flags.
pub mod writers {
    /// Streaming `BlockNote` JSON writer. Available when the `blocknote` feature is enabled.
    #[cfg(feature = "blocknote")]
    #[cfg_attr(docsrs, doc(cfg(feature = "blocknote")))]
    pub use docspec_blocknote_writer::BlockNoteWriter;
}

pub use docspec_core::*;

/// Streaming JSON emission primitives.
///
/// Lower-level building blocks for implementing custom JSON-based writers.
/// Re-exported from the `docspec-json` crate. Available when the `json` feature is enabled.
#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub use docspec_json as json;
