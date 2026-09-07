//! WebAssembly bindings for streaming document conversion.

extern crate alloc;

mod error;
mod formats;
#[cfg(conversion)]
mod host_io;

use wasm_bindgen::prelude::*;

#[cfg(conversion)]
use crate::formats::Input;
use crate::formats::{input_format, output_format};
#[cfg(conversion)]
use crate::host_io::{HostReader, HostWriter};

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_API: &str = r#"
export interface ReadAtSource {
  readonly size: number;
  readAt(offset: number, length: number): Uint8Array;
}
export type WriteChunk = (chunk: Uint8Array) => void;
export function convert_stream(
  from: string,
  to: string,
  source: ReadAtSource,
  write: WriteChunk,
): void;
"#;

/// Returns the canonical names of the compiled input formats in sorted order.
#[wasm_bindgen]
#[must_use]
pub fn input_formats() -> Vec<String> {
    formats::input_formats()
        .iter()
        .map(|format| String::from(*format))
        .collect()
}

/// Returns the canonical names of the compiled output formats in sorted order.
#[wasm_bindgen]
#[must_use]
pub fn output_formats() -> Vec<String> {
    formats::output_formats()
        .iter()
        .map(|format| String::from(*format))
        .collect()
}

/// Converts a document through synchronous, bounded host callbacks.
///
/// Each input and output transfer is at most 64 KiB. Bytes emitted before an
/// error are provisional and must be discarded by the caller.
///
/// # Errors
///
/// Returns a JavaScript `Error` with a stable `code` property when arguments
/// are invalid, a format is unavailable, host I/O fails, or conversion fails.
#[wasm_bindgen(skip_typescript)]
pub fn convert_stream(
    from: &str,
    to: &str,
    source: &JsValue,
    write: &JsValue,
) -> core::result::Result<(), JsValue> {
    let input = input_format(from)?;
    let output = output_format(to)?;
    #[cfg(conversion)]
    {
        let guard = host_io::HostIoGuard::register(source, write)?;
        let reader = match input {
            #[cfg(feature = "docx-reader")]
            Input::Docx => make_docx_reader(HostReader::new(guard.size())),
            #[cfg(feature = "html-reader")]
            Input::Html => make_html_reader(HostReader::new(guard.size())),
            #[cfg(feature = "markdown-reader")]
            Input::Markdown => make_markdown_reader(HostReader::new(guard.size())),
        }
        .map_err(|error| host_io::conversion_error(&error.to_string()))?;
        let finalizer = HostWriter::new();
        let writer = docspec::AnyWriter::new(output, finalizer.clone());
        docspec_core::pipe(reader, writer)
            .map_err(|error| host_io::conversion_error(&error.to_string()))?;
        finalizer
            .finish()
            .map_err(|error| host_io::conversion_error(&error.to_string()))?;
        host_io::finish_conversion()
    }
    #[cfg(not(conversion))]
    {
        let _arguments = (input, output, source, write);
        Err(error::js_error(
            error::ErrorCode::InvalidArgument,
            "conversion requires at least one compiled reader and writer",
        ))
    }
}

#[cfg(all(conversion, feature = "docx-reader"))]
fn make_docx_reader(reader: HostReader) -> docspec_core::Result<docspec::AnyReader> {
    docspec::AnyReader::from_reader_streaming(docspec::InputFormat::Docx, reader)
}

#[cfg(all(conversion, feature = "html-reader"))]
fn make_html_reader(reader: HostReader) -> docspec_core::Result<docspec::AnyReader> {
    docspec::AnyReader::from_reader(docspec::InputFormat::Html, reader)
}

#[cfg(all(conversion, feature = "markdown-reader"))]
fn make_markdown_reader(reader: HostReader) -> docspec_core::Result<docspec::AnyReader> {
    docspec::AnyReader::from_reader(docspec::InputFormat::Markdown, reader)
}

/// Converts a Markdown string to `BlockNote` JSON format.
///
/// # Errors
///
/// Returns a JavaScript error string if the conversion fails due to a
/// parse error, invalid event sequence, or JSON serialization error.
#[cfg(all(feature = "markdown-reader", feature = "blocknote-writer"))]
#[wasm_bindgen]
pub fn convert_markdown_to_blocknote(markdown: &str) -> core::result::Result<String, JsValue> {
    let reader = docspec::AnyReader::from_str(docspec::InputFormat::Markdown, markdown)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mut output = Vec::new();
    let sink = docspec::AnyWriter::new(docspec::OutputFormat::Blocknote, &mut output);
    docspec_core::pipe(reader, sink).map_err(|e| JsValue::from_str(&e.to_string()))?;
    String::from_utf8(output).map_err(|e| JsValue::from_str(&e.to_string()))
}
