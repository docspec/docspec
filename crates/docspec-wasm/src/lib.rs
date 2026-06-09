//! WebAssembly bindings for the `DocSpec` document conversion library.
//!
//! Exposes the markdown-to-`BlockNote` conversion pipeline for use in browser
//! environments via wasm-bindgen.

use wasm_bindgen::prelude::*;

/// Converts a Markdown string to `BlockNote` JSON format.
///
/// # Errors
///
/// Returns a JavaScript error string if the conversion fails due to a
/// parse error, invalid event sequence, or JSON serialization error.
#[wasm_bindgen]
pub fn convert_markdown_to_blocknote(markdown: &str) -> core::result::Result<String, JsValue> {
    let reader = docspec::AnyReader::from_str(docspec::InputFormat::Markdown, markdown)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mut output = Vec::new();
    let sink = docspec::AnyWriter::new(docspec::OutputFormat::Blocknote, &mut output);
    docspec_core::pipe(reader, sink).map_err(|e| JsValue::from_str(&e.to_string()))?;
    String::from_utf8(output).map_err(|e| JsValue::from_str(&e.to_string()))
}
