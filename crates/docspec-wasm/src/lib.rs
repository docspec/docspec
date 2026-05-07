//! WebAssembly bindings for the `DocSpec` document conversion library.
//!
//! Exposes the markdown-to-`BlockNote` conversion pipeline for use in browser
//! environments via wasm-bindgen.

use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::{Error, EventSink as _, EventSource as _, StackTrackingSink};
use docspec_markdown_reader::MarkdownReader;
use wasm_bindgen::prelude::*;

/// Converts a Markdown string to `BlockNote` JSON format.
///
/// # Errors
///
/// Returns a JavaScript error string if the conversion fails due to a
/// parse error, invalid event sequence, or JSON serialization error.
#[wasm_bindgen]
pub fn convert_markdown_to_blocknote(markdown: &str) -> core::result::Result<String, JsValue> {
    let mut reader = MarkdownReader::new(markdown);
    let mut output = Vec::new();
    let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut output));

    let mut next = reader.next_event();
    while let Ok(Some(event)) = next {
        writer
            .handle_event(event)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        next = reader.next_event();
    }
    next.map_err(|e| JsValue::from_str(&e.to_string()))?;
    writer
        .finish()
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    String::from_utf8(output)
        .map_err(|e| Error::Other {
            message: e.to_string(),
        })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
