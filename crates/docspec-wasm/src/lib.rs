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

#[cfg(test)]
mod tests {
    use super::convert_markdown_to_blocknote;

    #[test]
    fn heading_and_paragraph() {
        let result = convert_markdown_to_blocknote("# Hello\n\nWorld");
        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert_eq!(
            result.unwrap_or_default(),
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Hello","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"World","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn empty_input() {
        let result = convert_markdown_to_blocknote("");
        assert!(result.is_ok(), "empty input should succeed: {result:?}");
        assert_eq!(result.unwrap_or_default(), "[]");
    }

    #[test]
    fn plain_paragraph() {
        let result = convert_markdown_to_blocknote("Just a paragraph");
        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert_eq!(
            result.unwrap_or_default(),
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Just a paragraph","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bold_and_italic_formatting() {
        let result = convert_markdown_to_blocknote("**bold** and *italic*");
        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert_eq!(
            result.unwrap_or_default(),
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"bold","styles":{"bold":true}},{"type":"text","text":" and ","styles":{}},{"type":"text","text":"italic","styles":{"italic":true}}],"children":[]}]"#
        );
    }
}
