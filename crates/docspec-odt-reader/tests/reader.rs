//! Integration tests for `OdtReader`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

use docspec_core::{Event, EventSource as _};
use docspec_odt_reader::OdtReader;

#[test]
fn emits_exactly_an_empty_document() {
    let mut reader = OdtReader::new();
    assert_eq!(
        docspec_test_utils::collect_events(&mut reader),
        vec![
            Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            },
            Event::EndDocument,
        ]
    );
}

#[test]
fn stays_exhausted_after_end() {
    let mut reader = OdtReader::new();
    let _ = docspec_test_utils::collect_events(&mut reader);
    assert_eq!(reader.next_event().unwrap(), None);
}
