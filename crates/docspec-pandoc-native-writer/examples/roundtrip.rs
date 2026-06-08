//! Roundtrip example: writes a representative event sequence to stdout in Pandoc native format.
//!
//! Run with: `cargo run -p docspec-pandoc-native-writer --example roundtrip`
//! Then pipe through pandoc: `cargo run ... | pandoc -f native -t native`.

use docspec_core::{Event, EventSink as _, TextStyle};
use docspec_pandoc_native_writer::PandocNativeWriter;
use std::io;

fn main() -> io::Result<()> {
    let stdout = io::stdout();
    let mut writer = PandocNativeWriter::new(stdout.lock());

    let events = [
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        },
        Event::StartParagraph {
            alignment: None,
            id: None,
        },
        Event::Text {
            content: "Hello".to_string(),
            style: TextStyle::default(),
        },
        Event::Text {
            content: "\u{2019}".to_string(),
            style: TextStyle::default(),
        },
        Event::EndParagraph,
        Event::StartParagraph {
            alignment: None,
            id: None,
        },
        Event::Text {
            content: "\u{0e}Hello".to_string(),
            style: TextStyle::default(),
        },
        Event::Text {
            content: "\u{1}5".to_string(),
            style: TextStyle::default(),
        },
        Event::Text {
            content: "\u{0}".to_string(),
            style: TextStyle::default(),
        },
        Event::EndParagraph,
        Event::EndDocument,
    ];

    for event in events {
        writer
            .handle_event(event)
            .map_err(|e| io::Error::other(e.to_string()))?;
    }
    writer
        .finish()
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}
