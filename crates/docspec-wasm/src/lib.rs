//! WebAssembly bindings for the `DocSpec` document conversion library.
//!
//! Exposes the markdown-to-`BlockNote` conversion pipeline for use in browser
//! environments via wasm-bindgen.

use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::StackTrackingSink;
use docspec_markdown_reader::MarkdownReader;
use js_sys::Function;
use wasm_bindgen::prelude::*;

/// A `Write` implementation that forwards each chunk to a JavaScript callback function.
///
/// Converts JavaScript exceptions from the callback into `io::Error` to prevent them
/// from becoming WASM traps.
struct JsCallbackWriter<'a> {
    on_chunk: &'a Function,
}

impl std::io::Write for JsCallbackWriter<'_> {
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let chunk = js_sys::Uint8Array::from(buf);
        self.on_chunk
            .call1(&JsValue::NULL, &chunk)
            .map_err(|js_err| std::io::Error::other(format!("{js_err:?}")))?;
        Ok(buf.len())
    }
}

/// Converts a Markdown string to `BlockNote` JSON format.
///
/// # Errors
///
/// Returns a JavaScript error string if the conversion fails due to a
/// parse error, invalid event sequence, or JSON serialization error.
#[wasm_bindgen]
pub fn convert_markdown_to_blocknote(markdown: &str) -> core::result::Result<String, JsValue> {
    let reader = MarkdownReader::new(markdown);
    let mut output = Vec::new();
    let sink = StackTrackingSink::new(BlockNoteWriter::new(&mut output));
    docspec_core::pipe(reader, sink).map_err(|e| JsValue::from_str(&e.to_string()))?;
    String::from_utf8(output).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Converts a Markdown string to `BlockNote` JSON format, calling `on_chunk` for each
/// output chunk as it is produced.
///
/// Unlike [`convert_markdown_to_blocknote`], this variant does not buffer the output.
/// The `on_chunk` callback receives a `Uint8Array` for each chunk of JSON output.
///
/// # Errors
///
/// Returns a JavaScript error string if the conversion fails, or if `on_chunk` throws.
#[wasm_bindgen]
pub fn convert_markdown_to_blocknote_streaming(
    markdown: &str,
    on_chunk: &Function,
) -> core::result::Result<(), JsValue> {
    let mut writer_target = JsCallbackWriter { on_chunk };
    let reader = MarkdownReader::new(markdown);
    let writer = StackTrackingSink::new(BlockNoteWriter::new(&mut writer_target));
    docspec_core::pipe(reader, writer).map_err(|e| JsValue::from_str(&e.to_string()))
}
