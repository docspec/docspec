//! Probe test to verify struson 0.7.2 streaming string API.
//!
//! Confirmed API:
//! - `JsonWriter::string_value_writer()` returns a `StringValueWriter` impl
//! - `StringValueWriter` extends `Write` and has `finish_value()` method
//! - This allows streaming large string values without buffering

#![allow(clippy::unwrap_used)]

use struson::writer::{JsonStreamWriter, JsonWriter as _, StringValueWriter as _};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn struson_string_value_writer_exists_and_works() {
        // Create a stream writer with a Vec buffer
        let buffer = Vec::<u8>::new();
        let mut json_writer = JsonStreamWriter::new(buffer);

        // Open a string value using the streaming API
        let mut string_writer = json_writer.string_value_writer().unwrap();

        // Write bytes to the string writer (streaming — no internal Vec)
        string_writer.write_all(b"hello").unwrap();

        // Finish the string value (writes closing `"` to output)
        string_writer.finish_value().unwrap();

        // Finish the JSON document
        let output = json_writer.finish_document().unwrap();

        // Assert the output is the JSON-encoded string "hello" — 7 bytes: `"hello"`
        assert_eq!(output, b"\"hello\"");
    }
}
