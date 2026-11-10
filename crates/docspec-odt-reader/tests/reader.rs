//! Integration tests for `OdtReader`.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

use std::io::{Cursor, Write as _};

use docspec_core::{Error, Event, EventSource as _};
use docspec_odt_reader::OdtReader;
use docspec_test_utils::{
    collect_events, synth_docx_with_entries, synth_odt, try_collect_events, CompressionMethod,
};

fn content_xml(body: &str) -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<office:document-content"#,
            r#" xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0""#,
            r#" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0""#,
            r#" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0""#,
            r#" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0">"#,
            r#"<office:body><office:text>{}</office:text></office:body>"#,
            r#"</office:document-content>"#
        ),
        body
    )
}

fn reader(body: &str) -> OdtReader {
    OdtReader::from_reader(Cursor::new(synth_odt(&content_xml(body)))).unwrap()
}

/// The events between `StartDocument` and `EndDocument` for `body`.
fn body_events(body: &str) -> Vec<Event> {
    let mut events = collect_events(&mut reader(body));
    assert_eq!(events.remove(0), start_document(), "first event");
    assert_eq!(events.pop(), Some(Event::EndDocument), "last event");
    events
}

/// The message of the parse error reading `body` fails with.
fn parse_error(body: &str) -> String {
    match try_collect_events(&mut reader(body)) {
        Err(Error::Parse {
            message,
            position: None,
        }) => message,
        other => panic!("expected a parse error, got {other:?}"),
    }
}

fn start_document() -> Event {
    Event::StartDocument {
        id: None,
        language: None,
        metadata: None,
    }
}

fn start_paragraph() -> Event {
    Event::StartParagraph {
        alignment: None,
        id: None,
    }
}

fn text(content: &str) -> Event {
    Event::Text {
        content: content.to_string(),
    }
}

fn paragraph(content: &str) -> Vec<Event> {
    vec![start_paragraph(), text(content), Event::EndParagraph]
}

fn odt_file(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(bytes).unwrap();
    file
}

#[test]
fn empty_body_is_an_empty_document() {
    assert_eq!(
        collect_events(&mut reader("")),
        vec![start_document(), Event::EndDocument]
    );
}

#[test]
fn paragraphs_are_emitted_in_order() {
    assert_eq!(
        body_events("<text:p>one</text:p><text:p>two</text:p>"),
        [paragraph("one"), paragraph("two")].concat()
    );
}

#[test]
fn empty_paragraphs_have_no_text() {
    assert_eq!(
        body_events("<text:p/><text:p></text:p>"),
        vec![
            start_paragraph(),
            Event::EndParagraph,
            start_paragraph(),
            Event::EndParagraph,
        ]
    );
}

#[test]
fn spans_and_links_are_transparent() {
    assert_eq!(
        body_events(concat!(
            r#"<text:p>a<text:span text:style-name="T1">b</text:span>"#,
            r#"<text:a xlink:href="https://example.com/"><text:span>c</text:span></text:a></text:p>"#
        )),
        vec![
            start_paragraph(),
            text("a"),
            text("b"),
            text("c"),
            Event::EndParagraph,
        ]
    );
}

#[test]
fn space_elements_expand_to_their_count() {
    assert_eq!(
        body_events(r#"<text:p>a<text:s/>b<text:s text:c="3"/>c<text:s text:c="0"/>d</text:p>"#),
        vec![
            start_paragraph(),
            text("a"),
            text(" "),
            text("b"),
            text("   "),
            text("c"),
            text("d"),
            Event::EndParagraph,
        ]
    );
}

#[test]
fn tabs_and_line_breaks_are_emitted() {
    assert_eq!(
        body_events("<text:p>a<text:tab/>b<text:line-break/>c</text:p>"),
        vec![
            start_paragraph(),
            text("a"),
            text("\t"),
            text("b"),
            Event::LineBreak,
            text("c"),
            Event::EndParagraph,
        ]
    );
}

#[test]
fn references_and_cdata_become_text() {
    assert_eq!(
        body_events("<text:p>a&amp;b&#x41;<![CDATA[<c>]]></text:p>"),
        vec![
            start_paragraph(),
            text("a"),
            text("&"),
            text("b"),
            text("A"),
            text("<c>"),
            Event::EndParagraph,
        ]
    );
}

#[test]
fn paragraphs_in_lists_tables_and_sections_are_flattened() {
    assert_eq!(
        body_events(concat!(
            "<text:list><text:list-item><text:p>item</text:p></text:list-item></text:list>",
            "<table:table><table:table-row><table:table-cell><text:p>cell</text:p>",
            "</table:table-cell></table:table-row></table:table>",
            "<text:section><text:p>section</text:p></text:section>"
        )),
        [paragraph("item"), paragraph("cell"), paragraph("section")].concat()
    );
}

#[test]
fn headings_and_text_outside_paragraphs_are_dropped() {
    assert_eq!(
        body_events(
            r#"<text:h text:outline-level="1">Title &amp; <![CDATA[more]]></text:h><text:p>body</text:p>"#
        ),
        paragraph("body")
    );
}

#[test]
fn tracked_changes_are_dropped() {
    assert_eq!(
        body_events(concat!(
            r#"<text:tracked-changes><text:changed-region><text:deletion>"#,
            r#"<text:p>deleted &amp; gone</text:p></text:deletion></text:changed-region>"#,
            r#"</text:tracked-changes><text:p>kept</text:p>"#
        )),
        paragraph("kept")
    );
}

#[test]
fn notes_and_frames_inside_a_paragraph_are_dropped() {
    assert_eq!(
        body_events(concat!(
            r#"<text:p>a<text:note><text:note-citation>1</text:note-citation>"#,
            r#"<text:note-body><text:p>footnote</text:p></text:note-body></text:note>"#,
            r#"<draw:frame><draw:text-box><text:p>boxed</text:p></draw:text-box></draw:frame>"#,
            r#"b</text:p>"#
        )),
        vec![start_paragraph(), text("a"), text("b"), Event::EndParagraph]
    );
}

#[test]
fn stays_exhausted_after_end() {
    let mut reader = reader("<text:p>a</text:p>");
    let _ = collect_events(&mut reader);
    assert_eq!(reader.next_event().unwrap(), None);
}

#[test]
fn debug_names_the_reader() {
    assert_eq!(
        format!("{:?}", reader("")),
        "OdtReader { phase: NotStarted, .. }"
    );
}

#[test]
fn from_path_matches_from_reader() {
    let bytes = synth_odt(&content_xml("<text:p>one</text:p><text:p>two</text:p>"));
    let file = odt_file(&bytes);
    let from_path = collect_events(&mut OdtReader::from_path(file.path()).unwrap());
    let from_reader = collect_events(&mut OdtReader::from_reader(Cursor::new(bytes)).unwrap());
    assert_eq!(from_path, from_reader);
    assert_eq!(
        from_path,
        [
            vec![start_document()],
            paragraph("one"),
            paragraph("two"),
            vec![Event::EndDocument],
        ]
        .concat()
    );
}

#[test]
fn from_reader_rejects_non_zip_input() {
    let Err(Error::Parse { message, position }) = OdtReader::from_reader(Cursor::new(b"anything"))
    else {
        panic!("expected a parse error");
    };
    assert_eq!(message, "not a valid ZIP archive");
    assert_eq!(position, None);
}

#[test]
fn from_reader_requires_content_xml() {
    let bytes = synth_docx_with_entries(&[("mimetype", CompressionMethod::Stored, b"x")]);
    let Err(Error::Parse { message, position }) = OdtReader::from_reader(Cursor::new(bytes)) else {
        panic!("expected a parse error");
    };
    assert_eq!(message, "malformed content.xml: entry is missing");
    assert_eq!(position, None);
}

#[test]
fn from_path_missing_file_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let Err(Error::Io { source }) = OdtReader::from_path(dir.path().join("missing.odt")) else {
        panic!("expected an I/O error");
    };
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn from_path_directory_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let Err(Error::Io { source }) = OdtReader::from_path(dir.path()) else {
        panic!("expected an I/O error");
    };
    assert_eq!(source.kind(), std::io::ErrorKind::IsADirectory);
}

#[test]
fn from_path_corrupt_stream_is_an_io_error() {
    let mut bytes = synth_docx_with_entries(&[(
        "content.xml",
        CompressionMethod::Deflated,
        content_xml("<text:p>one</text:p>").as_bytes(),
    )]);
    // The entry's data follows its 30-byte local header and 11-byte name; a
    // deflate stream cannot start with the reserved block type 0b11.
    bytes[41] = 0xFF;
    let file = odt_file(&bytes);
    let mut reader = OdtReader::from_path(file.path()).unwrap();
    let Err(Error::Io { source }) = try_collect_events(&mut reader) else {
        panic!("expected an I/O error");
    };
    assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn eof_inside_a_paragraph_is_a_parse_error() {
    let bytes = synth_odt("<text:p>unfinished");
    let mut reader = OdtReader::from_reader(Cursor::new(bytes)).unwrap();
    let Err(Error::Parse { message, position }) = try_collect_events(&mut reader) else {
        panic!("expected a parse error");
    };
    assert_eq!(message, "malformed content.xml: unexpected end of file");
    assert_eq!(position, None);
}

#[test]
fn malformed_xml_is_a_parse_error() {
    assert_eq!(
        parse_error("<text:p>a<!-x</text:p>"),
        "malformed content.xml: syntax error: comment not closed: `-->` not found before end of input"
    );
}

#[test]
fn invalid_space_count_is_a_parse_error() {
    assert_eq!(
        parse_error(r#"<text:p><text:s text:c="many"/></text:p>"#),
        "malformed content.xml: invalid text:c"
    );
    assert_eq!(
        parse_error(r#"<text:p><text:s text:c="70000"/></text:p>"#),
        "malformed content.xml: invalid text:c"
    );
}

#[test]
fn malformed_space_attribute_is_a_parse_error() {
    assert_eq!(
        parse_error("<text:p><text:s text:c=3/></text:p>"),
        "malformed content.xml: position 14: attribute value must be enclosed in `\"` or `'`"
    );
}

#[test]
fn unknown_entity_is_a_parse_error() {
    assert_eq!(
        parse_error("<text:p>&nope;</text:p>"),
        "malformed content.xml: at 1..5: unrecognized entity `nope`"
    );
}

#[test]
fn invalid_utf8_is_a_parse_error() {
    for body in [
        &b"<text:p>\xFF</text:p>"[..],
        b"<text:p><![CDATA[\xFF]]></text:p>",
        b"<text:p>&\xFF;</text:p>",
    ] {
        let bytes = synth_docx_with_entries(&[("content.xml", CompressionMethod::Stored, body)]);
        let mut reader = OdtReader::from_reader(Cursor::new(bytes)).unwrap();
        let Err(Error::Parse { message, position }) = try_collect_events(&mut reader) else {
            panic!("expected a parse error");
        };
        assert_eq!(
            message,
            "malformed content.xml: cannot decode input using UTF-8: invalid utf-8 sequence of 1 bytes from index 0"
        );
        assert_eq!(position, None);
    }
}

#[test]
fn reads_a_document_saved_by_libreoffice() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/odt/docspec/paragraphs.odt"
    );
    let from_path = collect_events(&mut OdtReader::from_path(path).unwrap());
    let bytes = std::fs::read(path).unwrap();
    let from_reader = collect_events(&mut OdtReader::from_reader(Cursor::new(bytes)).unwrap());
    assert_eq!(from_path, from_reader);
    assert_eq!(
        from_path,
        [
            vec![start_document()],
            paragraph("Heading"),
            vec![
                start_paragraph(),
                text("First paragraph with "),
                text("bold"),
                text(" and "),
                text("a link"),
                text("."),
                Event::EndParagraph,
                start_paragraph(),
                text("Second paragraph "),
                text("&"),
                text(" an ampersand."),
                Event::LineBreak,
                text("After a line break."),
                Event::EndParagraph,
            ],
            paragraph("List item"),
            paragraph("Cell"),
            paragraph("Last paragraph."),
            vec![Event::EndDocument],
        ]
        .concat()
    );
}
