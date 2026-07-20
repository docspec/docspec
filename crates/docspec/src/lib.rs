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
//! | `docx`         | DOCX (paragraphs, headings, tables, lists, hyperlinks, images, run styles)  | `readers::DocxReader`       |
//!
//! [`readers::DocxReader`] is dispatched through [`AnyReader::from_reader`] and
//! [`AnyReader::from_path`]. Use `AnyReader::from_path(InputFormat::Docx, path)` to
//! open a DOCX file, or `AnyReader::from_reader(InputFormat::Docx, cursor)` to read
//! from an in-memory buffer.
//!
//! ## Writers
//!
//! | Feature             | Format                  | Re-exports                    |
//! |---------------------|-------------------------|-------------------------------|
//! | `blocknote-writer`  | `BlockNote` JSON        | [`writers::BlockNoteWriter`]  |
//! | `oxa-writer`        | `oxa.dev` JSON          | `writers::OxaWriter`          |
//! | `html-writer`       | HTML (paragraphs only)  | `writers::HtmlWriter`         |
//! | `pandoc-native-writer` | Pandoc native block list | `writers::PandocNativeWriter` |
//! | `markdown-writer`   | Markdown (paragraphs and headings only) | `writers::MarkdownWriter` |
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
//! | `pandoc-native` | Pandoc native in both directions (writer only until reader lands) |
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
//! Convert Markdown to `BlockNote` JSON:
//!
//! ```no_run
//! # #[cfg(all(feature = "markdown", feature = "blocknote-writer"))]
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use docspec::readers::MarkdownReader;
//! use docspec::writers::BlockNoteWriter;
//! use docspec::{EventSink, EventSource, StackTrackingSink};
//!
//! let markdown = "# Hello\n\nWorld";
//! let mut reader = MarkdownReader::from_str(markdown);
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

    /// Streaming DOCX reader. Available when the `docx` feature is enabled.
    /// Dispatched through [`crate::AnyReader::from_reader`] and
    /// [`crate::AnyReader::from_path`]. Emits paragraphs, headings, block quotes,
    /// preformatted blocks, tables, ordered/unordered lists, hyperlinks, images
    /// (`DrawingML` and VML, streamed via `AssetHandle`), and inline run styles —
    /// bold, italic, underline, strikethrough, sub/superscript, plus text color,
    /// highlight, and shading. Comments, footnotes, headers/footers, document
    /// metadata, and tracked deletions are silently dropped; see
    /// [`docspec-docx-reader`](https://docs.rs/docspec-docx-reader) for the
    /// authoritative supported and out-of-scope list.
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

    /// Streaming Pandoc native block-list writer. Available when the
    /// `pandoc-native-writer` feature is enabled (either directly, or via the
    /// `pandoc-native` meta feature).
    #[cfg(feature = "pandoc-native-writer")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pandoc-native-writer")))]
    pub use docspec_pandoc_native_writer::PandocNativeWriter;

    /// Streaming Markdown (`CommonMark`) writer — paragraphs and headings only.
    /// Available when the `markdown-writer` feature is enabled.
    #[cfg(feature = "markdown-writer")]
    #[cfg_attr(docsrs, doc(cfg(feature = "markdown-writer")))]
    pub use docspec_markdown_writer::MarkdownWriter;
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
