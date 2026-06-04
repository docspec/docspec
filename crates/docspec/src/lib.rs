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
//! | Feature        | Format                                              | Re-exports                  |
//! |----------------|-----------------------------------------------------|-----------------------------|
//! | `markdown`     | Markdown (`CommonMark` + GFM tables/strikethrough)  | [`readers::MarkdownReader`] |
//! | `html`         | HTML (paragraphs only)                              | `readers::HtmlReader`       |
//! | `docx`         | DOCX (paragraphs and text only)                     | `readers::DocxReader`       |
//!
//! ## Writers
//!
//! | Feature             | Format                  | Re-exports                    |
//! |---------------------|-------------------------|-------------------------------|
//! | `blocknote-writer`  | `BlockNote` JSON        | [`writers::BlockNoteWriter`]  |
//! | `oxa-writer`        | `oxa.dev` JSON          | `writers::OxaWriter`          |
//! | `html-writer`       | HTML (paragraphs only)  | `writers::HtmlWriter`         |
//!
//! ## Primitives
//!
//! | Feature | Re-exports                                                            |
//! |---------|-----------------------------------------------------------------------|
//! | `json`  | `json` — streaming JSON emission primitives for custom writers        |
//!
//! ## Convenience
//!
//! | Feature       | Enables                                                          |
//! |---------------|------------------------------------------------------------------|
//! | `blocknote`   | `BlockNote` in both directions (writer only until reader lands)  |
//! | `oxa`         | `oxa.dev` in both directions (writer only until reader lands)    |
//! | `all-readers` | All reader features                                              |
//! | `all-writers` | All writer features                                              |
//! | `all-libs`    | All primitive/library features (currently `json`)                |
//! | `full`        | Everything (`all-readers` + `all-writers` + `all-libs`)          |
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
//! ## Unified Reader Factory
//!
//! [`AnyReader`] is the single entry point for all input formats. Use
//! [`AnyReader::from_path`] when you have a file path, or
//! [`AnyReader::from_reader`] when you have any `Read + Seek` source (a file,
//! a network buffer, an in-memory cursor, etc.).
//!
//! Convert Markdown to `BlockNote` JSON via the unified factory:
//!
//! ```no_run
//! # #[cfg(all(feature = "markdown", feature = "blocknote-writer"))]
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use std::io::Cursor;
//! use docspec::{AnyReader, InputFormat};
//! use docspec::writers::BlockNoteWriter;
//! use docspec::{EventSink, EventSource, StackTrackingSink};
//!
//! let markdown = "# Hello\n\nWorld";
//! let cursor = Cursor::new(markdown.as_bytes().to_vec());
//! let mut reader = AnyReader::from_reader(InputFormat::Markdown, cursor)?;
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
//!
//! Or open a file directly by path:
//!
//! ```no_run
//! # #[cfg(all(feature = "markdown", feature = "blocknote-writer"))]
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use docspec::{AnyReader, InputFormat};
//! use docspec::writers::BlockNoteWriter;
//! use docspec::{EventSink, EventSource, StackTrackingSink};
//!
//! let mut reader = AnyReader::from_path(InputFormat::Markdown, "input.md")?;
//! let mut buf = Vec::<u8>::new();
//! let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
//!
//! while let Some(event) = reader.next_event()? {
//!     writer.handle_event(event)?;
//! }
//! writer.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## DOCX Support
//!
//! DOCX is a binary format. Enable the `docx` feature and pass a file path or
//! any `Read + Seek` source. The reader emits paragraphs and text only; styles,
//! tables, lists, images, headers/footers, and tracked changes are silently
//! dropped.
//!
//! ```no_run
//! # #[cfg(all(feature = "docx", feature = "blocknote-writer"))]
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use docspec::{AnyReader, InputFormat};
//! use docspec::writers::BlockNoteWriter;
//! use docspec::{EventSink, EventSource, StackTrackingSink};
//!
//! let mut reader = AnyReader::from_path(InputFormat::Docx, "doc.docx")?;
//! let mut buf = Vec::<u8>::new();
//! let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));
//!
//! while let Some(event) = reader.next_event()? {
//!     writer.handle_event(event)?;
//! }
//! writer.finish()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Migrating from 1.x
//!
//! The 1.x `AnyReader::new(format, &str)` constructor remains available for
//! in-memory text inputs so existing Markdown and HTML callers can migrate
//! gradually. New code should prefer the fallible owned-source constructors:
//!
//! ```no_run
//! # #[cfg(feature = "markdown")]
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use std::io::Cursor;
//! use docspec::{AnyReader, InputFormat};
//!
//! // From owned bytes:
//! let text = "# Hello\n\nWorld";
//! let mut reader = AnyReader::from_reader(
//!     InputFormat::Markdown,
//!     Cursor::new(text.as_bytes().to_vec()),
//! )?;
//!
//! // From a file path:
//! let mut reader = AnyReader::from_path(InputFormat::Markdown, "input.md")?;
//! # Ok(())
//! # }
//! ```
//!
//! Use `from_reader` or `from_path` for DOCX; `AnyReader::new` accepts text
//! input only. Text readers (Markdown, HTML) still read the full input string
//! into memory before parsing. The unified factory is about a consistent
//! user-facing API, not about making text parsing incremental.

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

    /// Streaming DOCX reader. Available when the `docx` feature is enabled.
    /// Takes a file path or `Read + Seek` source. Emits only paragraphs and text;
    /// styles, tables, lists, images, headers/footers, metadata, and tracked changes
    /// are silently dropped.
    #[cfg(feature = "docx")]
    #[cfg_attr(docsrs, doc(cfg(feature = "docx")))]
    pub use docspec_docx_reader::DocxReader;
}

/// Document writers (event sinks).
///
/// Each writer is gated by a feature flag. See the crate-level documentation for the
/// full list of supported formats and corresponding feature flags.
pub mod writers {
    /// Streaming `BlockNote` JSON writer. Available when the `blocknote-writer` feature
    /// is enabled (either directly, or via the `blocknote` meta feature).
    #[cfg(feature = "blocknote-writer")]
    #[cfg_attr(docsrs, doc(cfg(feature = "blocknote-writer")))]
    pub use docspec_blocknote_writer::BlockNoteWriter;

    /// Streaming `oxa.dev` JSON writer. Available when the `oxa-writer` feature is
    /// enabled (either directly, or via the `oxa` meta feature).
    #[cfg(feature = "oxa-writer")]
    #[cfg_attr(docsrs, doc(cfg(feature = "oxa-writer")))]
    pub use docspec_oxa_writer::OxaWriter;

    /// Streaming HTML5 writer. Available when the `html-writer` feature is enabled.
    /// Note: currently emits only `<html>/<body>/<p>` and text within paragraphs;
    /// other events are silently ignored.
    #[cfg(feature = "html-writer")]
    #[cfg_attr(docsrs, doc(cfg(feature = "html-writer")))]
    pub use docspec_html_writer::HtmlWriter;
}

/// Format detection and conversion helpers.
///
/// Provides enums for input and output formats, plus functions to detect formats
/// from file paths based on extension.
pub mod format;

/// Enum-dispatch factories for document readers and writers.
pub mod factory;

pub use docspec_core::*;
pub use factory::reader::AnyReader;
pub use factory::writer::AnyWriter;
pub use format::{detect_input_format, detect_output_format, InputFormat, OutputFormat};

/// Streaming JSON emission primitives.
///
/// Lower-level building blocks for implementing custom JSON-based writers.
/// Re-exported from the `docspec-json` crate. Available when the `json` feature is enabled.
#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub use docspec_json as json;
