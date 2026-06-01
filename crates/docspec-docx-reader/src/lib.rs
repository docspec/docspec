//! DOCX to `DocSpec` event stream reader.
//!
//! This crate provides a [`DocxReader`] that implements [`EventSource`] to convert
//! DOCX (Office Open XML) documents into the `DocSpec` event stream format. It uses
//! `quick-xml`'s namespace-aware streaming pull parser and the `zip` crate to open
//! the OPC/ZIP container.
//!
//! # Quick Start
//!
//! ```
//! use docspec_docx_reader::{DocxReader, EventSource};
//! use std::io::Cursor;
//!
//! # fn example() -> docspec_core::Result<()> {
//! let docx_bytes: &[u8] = &[]; // your DOCX bytes here
//! let mut reader = DocxReader::new(Cursor::new(docx_bytes))?;
//!
//! while let Some(event) = reader.next_event()? {
//!     println!("{event:?}");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Supported Elements
//!
//! - `<w:document>` body → `StartDocument` / `EndDocument`
//! - `<w:p>` → `StartParagraph` / `EndParagraph` (alignment always `None`, id always `None`)
//! - `<w:r><w:t>...</w:t></w:r>` → `Text { content, style: TextStyle::default() }`
//! - `<w:tab/>` → `Text { content: "\t", style: TextStyle::default() }`
//! - `<w:br/>` (no type or `textWrapping`) → `LineBreak`
//!
//! # Unsupported Elements
//!
//! The following elements are silently skipped (no events emitted):
//!
//! - All run/paragraph properties (`<w:rPr>`, `<w:pPr>`, `<w:pStyle>`)
//! - Tables (`<w:tbl>`) and their contents (including inner `<w:p>`)
//! - Content controls (`<w:sdt>`) and their contents (including inner `<w:p>`)
//! - Page and column breaks (`<w:br w:type="page"/>`, `<w:br w:type="column"/>`)
//! - Section properties (`<w:sectPr>`)
//! - Images, hyperlinks, footnotes, comments, fields, bookmarks
//!
//! # Memory Model
//!
//! DOCX is an Open Packaging Convention (ZIP) container. The ZIP central directory
//! lives at the end of the file, which forces `Read + Seek` input and prevents
//! forward-only streaming over the package itself. After construction, the main
//! document XML is buffered into a `Vec<u8>` so the `ZipArchive` handle can be
//! released and event emission can proceed without lifetime constraints. Event
//! emission itself is still streaming — events are produced one at a time and
//! never accumulate.
//!
//! See [MANIFESTO.md] §'What We Owe to Each Other' for the bounded-buffer policy.
//!
//! # Resource Limits
//!
//! This v0 reader does NOT enforce resource limits. A maliciously crafted DOCX
//! containing a very large `document.xml` may exhaust available memory. A
//! zip-bomb with millions of small entries is not specifically rejected (the
//! `zip` crate's central directory parse provides minimal defense). Callers
//! processing untrusted input must apply their own size limits at the source
//! level (e.g., HTTP request body limit). See [SECURITY.md] §4 for the project's
//! resource-limit philosophy. A future iteration will close this gap.
//!
//! TODO(v1): resource limits per SECURITY.md §4
//!
//! # Conformance
//!
//! Accepts both ECMA-376 1st edition (Transitional) and 2nd-edition (Strict)
//! `WordprocessingML` namespaces. Main document part discovery is performed via
//! the `_rels/.rels` package relationship (the part name `word/document.xml`
//! is a convention, not a requirement, per ECMA-376 §11.3.10).

extern crate alloc;

mod discovery;
mod oox;

use core::marker::PhantomData;
use std::io::{Read, Seek};

pub use docspec_core::EventSource;
use docspec_core::{Event, Result, TextStyle};

enum NextEvent {
    Continue,
    Ready(Option<Event>),
}

#[derive(Clone, Copy)]
enum DepthEventKind {
    EndSdt,
    EndTable,
    StartSdt,
    StartTable,
}

/// Document processing phase.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// A fatal error occurred; stream is terminated.
    Failed,
    /// `EndDocument` has been emitted.
    Finished,
    /// `StartDocument` not yet emitted.
    NotStarted,
    /// Processing events between `StartDocument` and `EndDocument`.
    Running,
}

/// Content emission state for the current text element.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TextElementState {
    /// At least one text event has been emitted for the current text element.
    Emitted,
    /// No text event has been emitted for the current text element.
    Empty,
}

/// A streaming DOCX reader that implements [`EventSource`].
///
/// Parses DOCX files (Office Open XML) and emits a stream of [`Event`]s.
/// The reader processes the document incrementally without buffering the entire
/// file into memory.
pub struct DocxReader<R: Read + Seek> {
    /// Retains the generic type parameter R after the archive is dropped.
    _phantom: PhantomData<R>,
    /// Temporary buffer for XML content.
    buf: alloc::vec::Vec<u8>,
    /// Whether we are currently inside a paragraph element.
    in_paragraph: bool,
    /// Whether we are currently inside a run element.
    in_run: bool,
    /// Whether we are currently inside a text element.
    in_text: bool,
    /// Path to the main document part within the ZIP archive.
    main_part_path: alloc::string::String,
    /// Queue of events pending emission.
    pending: alloc::collections::VecDeque<Event>,
    /// Current processing phase.
    ///
    /// Once `phase` becomes `Phase::Finished` or `Phase::Failed`, it stays there.
    /// All subsequent `next_event` calls return `Ok(None)`.
    phase: Phase,
    /// Nesting depth of structured data tag (SDT) elements.
    sdt_depth: u32,
    /// Nesting depth of table elements.
    table_depth: u32,
    /// Content state for the current text element.
    text_state: TextElementState,
    /// Namespace-aware XML reader for the document.xml content.
    xml: quick_xml::reader::NsReader<std::io::Cursor<alloc::vec::Vec<u8>>>,
}

impl<R: Read + Seek> DocxReader<R> {
    fn handle_empty_flags(&mut self, is_paragraph: bool, is_text: bool) -> Option<Event> {
        if is_paragraph && self.table_depth == 0 && self.sdt_depth == 0 {
            self.pending.push_back(Event::EndParagraph);
            return Some(Event::StartParagraph {
                alignment: None,
                id: None,
            });
        }
        if is_text && self.in_run && self.in_paragraph {
            // NOTE: empty <w:t> emits Text { content: "" } — 1:1 mapping policy
            return Some(Event::Text {
                content: String::new(),
                style: TextStyle::default(),
            });
        }
        None
    }

    fn handle_end_flags(
        &mut self,
        is_paragraph: bool,
        is_run: bool,
        is_text: bool,
    ) -> Option<Event> {
        if is_paragraph && self.in_paragraph {
            self.in_paragraph = false;
            return Some(Event::EndParagraph);
        }
        if is_run && self.in_run {
            self.in_run = false;
        }
        if is_text && self.in_text {
            self.in_text = false;
            if self.text_state == TextElementState::Empty {
                // NOTE: empty <w:t> emits Text { content: "" } — 1:1 mapping policy
                return Some(Event::Text {
                    content: String::new(),
                    style: TextStyle::default(),
                });
            }
        }
        None
    }

    fn handle_start_flags(
        &mut self,
        is_paragraph: bool,
        is_run: bool,
        is_text: bool,
    ) -> Option<Event> {
        if is_paragraph && self.table_depth == 0 && self.sdt_depth == 0 {
            self.in_paragraph = true;
            return Some(Event::StartParagraph {
                alignment: None,
                id: None,
            });
        }
        if is_run && self.in_paragraph {
            self.in_run = true;
        }
        if is_text && self.in_run && self.in_paragraph {
            self.in_text = true;
            self.text_state = TextElementState::Empty;
        }
        None
    }

    /// Returns the path to the main document part within the ZIP archive.
    #[inline]
    #[must_use]
    pub fn main_part_path(&self) -> &str {
        &self.main_part_path
    }

    /// Creates a new DOCX reader from a source.
    ///
    /// # Errors
    ///
    /// Returns an error if the source cannot be read or is not a valid DOCX file.
    #[inline]
    pub fn new(reader: R) -> Result<Self> {
        let mut archive = zip::ZipArchive::new(reader).map_err(|e| docspec_core::Error::Other {
            message: format!("failed to open ZIP archive: {e}"),
        })?;
        let main_part_path = discovery::discover_main_part(&mut archive)?;
        let mut entry =
            archive
                .by_name(&main_part_path)
                .map_err(|_zip_err| docspec_core::Error::Parse {
                    message: format!("main document part '{main_part_path}' not found in package"),
                    position: None,
                })?;
        let capacity = usize::try_from(entry.size()).unwrap_or(0);
        let mut bytes = alloc::vec::Vec::with_capacity(capacity);
        std::io::copy(&mut entry, &mut bytes).map_err(|e| docspec_core::Error::Io { source: e })?;
        drop(entry);
        drop(archive);
        let mut xml = quick_xml::reader::NsReader::from_reader(std::io::Cursor::new(bytes));
        xml.config_mut().trim_text(false);
        xml.config_mut().expand_empty_elements = false;
        Ok(Self {
            _phantom: core::marker::PhantomData,
            buf: alloc::vec::Vec::new(),
            in_paragraph: false,
            in_run: false,
            in_text: false,
            main_part_path,
            pending: alloc::collections::VecDeque::new(),
            phase: Phase::NotStarted,
            sdt_depth: 0,
            table_depth: 0,
            text_state: TextElementState::Empty,
            xml,
        })
    }

    fn next_phase_event(&mut self) -> NextEvent {
        if self.phase == Phase::Failed || self.phase == Phase::Finished {
            return NextEvent::Ready(None);
        }
        if let Some(ev) = self.pending.pop_front() {
            return NextEvent::Ready(Some(ev));
        }
        if self.phase == Phase::NotStarted {
            self.phase = Phase::Running;
            return NextEvent::Ready(Some(start_document_event()));
        }
        NextEvent::Continue
    }

    #[cfg(test)]
    pub(crate) fn xml_bytes(&self) -> &[u8] {
        self.xml.get_ref().get_ref()
    }
}

impl<R: Read + Seek> EventSource for DocxReader<R> {
    /// Returns the next event from the stream, or `None` if the stream has ended.
    ///
    /// # Errors
    ///
    /// Returns an error if the source encounters a fatal problem.
    #[inline]
    fn next_event(&mut self) -> Result<Option<Event>> {
        if let NextEvent::Ready(event) = self.next_phase_event() {
            return Ok(event);
        }

        loop {
            self.buf.clear();
            let text_context = (
                self.in_text,
                self.in_paragraph,
                self.table_depth,
                self.sdt_depth,
            );
            match self.xml.read_resolved_event_into(&mut self.buf) {
                Err(e) => {
                    self.phase = Phase::Failed;
                    return Err(parse_error(format!("XML parse error: {e}")));
                }
                Ok((_, quick_xml::events::Event::Eof)) => {
                    self.phase = Phase::Finished;
                    return Ok(Some(Event::EndDocument));
                }
                Ok((_, quick_xml::events::Event::Text(text)))
                    if is_unhandled_non_whitespace_text(text_context, &text) =>
                {
                    self.phase = Phase::Failed;
                    return Err(parse_error(
                        "XML parse error: non-whitespace text outside handled elements".to_string(),
                    ));
                }
                Ok((ns, ev)) => {
                    use quick_xml::events::Event as XmlEvent;

                    if let Some(depth_event) = depth_event_kind(&ns, &ev) {
                        apply_depth_event(&mut self.table_depth, &mut self.sdt_depth, depth_event);
                        continue;
                    }

                    match ev {
                        XmlEvent::Start(e) => {
                            let (is_paragraph, is_run, is_text) =
                                w_element_flags(&ns, e.local_name().as_ref());
                            if let Some(event) =
                                self.handle_start_flags(is_paragraph, is_run, is_text)
                            {
                                return Ok(Some(event));
                            }
                        }
                        XmlEvent::End(e) => {
                            let (is_paragraph, is_run, is_text) =
                                w_element_flags(&ns, e.local_name().as_ref());
                            if let Some(event) =
                                self.handle_end_flags(is_paragraph, is_run, is_text)
                            {
                                return Ok(Some(event));
                            }
                        }
                        XmlEvent::Text(t) if self.in_text => {
                            let event = text_event_from_xml(&t).inspect_err(|_err| {
                                self.phase = Phase::Failed;
                            })?;
                            self.text_state = TextElementState::Emitted;
                            return Ok(Some(event));
                        }
                        XmlEvent::Empty(e)
                            if is_w_tab(&ns, e.local_name().as_ref())
                                && self.in_run
                                && self.in_paragraph =>
                        {
                            return Ok(Some(Event::Text {
                                content: "\t".to_string(),
                                style: TextStyle::default(),
                            }));
                        }
                        XmlEvent::Empty(e)
                            if is_w_br(&ns, e.local_name().as_ref())
                                && self.in_run
                                && self.in_paragraph =>
                        {
                            let br_type = br_type_attribute(&self.xml, &e).inspect_err(|_err| {
                                self.phase = Phase::Failed;
                            })?;
                            if matches!(br_type.as_deref(), None | Some(b"textWrapping")) {
                                return Ok(Some(Event::LineBreak));
                            }
                        }
                        XmlEvent::Empty(e) => {
                            let (is_paragraph, _, is_text) =
                                w_element_flags(&ns, e.local_name().as_ref());
                            if let Some(event) = self.handle_empty_flags(is_paragraph, is_text) {
                                return Ok(Some(event));
                            }
                        }
                        XmlEvent::Text(_)
                        | XmlEvent::CData(_)
                        | XmlEvent::Comment(_)
                        | XmlEvent::Decl(_)
                        | XmlEvent::PI(_)
                        | XmlEvent::DocType(_)
                        | XmlEvent::Eof => {}
                    }
                }
            }
        }
    }
}

fn depth_event_kind(
    ns: &quick_xml::name::ResolveResult<'_>,
    ev: &quick_xml::events::Event<'_>,
) -> Option<DepthEventKind> {
    use quick_xml::events::Event as XmlEvent;

    match ev {
        // <w:tbl> depth tracking — MUST come before paragraph arms
        // Decision: use raw u32 with saturating_add/sub for simplicity; see CODING_STANDARDS §14
        XmlEvent::Start(e) if is_w_table(ns, e.local_name().as_ref()) => {
            Some(DepthEventKind::StartTable)
        }
        XmlEvent::End(e) if is_w_table(ns, e.local_name().as_ref()) => {
            Some(DepthEventKind::EndTable)
        }
        // <w:sdt> depth tracking — MUST come before paragraph arms
        XmlEvent::Start(e) if is_w_sdt(ns, e.local_name().as_ref()) => {
            Some(DepthEventKind::StartSdt)
        }
        XmlEvent::End(e) if is_w_sdt(ns, e.local_name().as_ref()) => Some(DepthEventKind::EndSdt),
        XmlEvent::Start(_)
        | XmlEvent::End(_)
        | XmlEvent::Empty(_)
        | XmlEvent::Text(_)
        | XmlEvent::CData(_)
        | XmlEvent::Comment(_)
        | XmlEvent::Decl(_)
        | XmlEvent::PI(_)
        | XmlEvent::DocType(_)
        | XmlEvent::Eof => None,
    }
}

fn apply_depth_event(table_depth: &mut u32, sdt_depth: &mut u32, depth_event: DepthEventKind) {
    match depth_event {
        DepthEventKind::StartTable => *table_depth = (*table_depth).saturating_add(1),
        DepthEventKind::EndTable => *table_depth = (*table_depth).saturating_sub(1),
        DepthEventKind::StartSdt => *sdt_depth = (*sdt_depth).saturating_add(1),
        DepthEventKind::EndSdt => *sdt_depth = (*sdt_depth).saturating_sub(1),
    }
}

fn is_unhandled_non_whitespace_text(
    state: (bool, bool, u32, u32),
    text: &quick_xml::events::BytesText<'_>,
) -> bool {
    let (in_text, in_paragraph, table_depth, sdt_depth) = state;
    !in_text
        && !in_paragraph
        && table_depth == 0
        && sdt_depth == 0
        && text.as_ref().iter().any(|byte| !byte.is_ascii_whitespace())
}

fn br_type_attribute(
    xml: &quick_xml::reader::NsReader<std::io::Cursor<alloc::vec::Vec<u8>>>,
    e: &quick_xml::events::BytesStart<'_>,
) -> Result<Option<alloc::vec::Vec<u8>>> {
    for attr_result in e.attributes() {
        let attr = attr_result.map_err(|attr_err| docspec_core::Error::Parse {
            message: format!("attribute error: {attr_err}"),
            position: None,
        })?;
        let (attr_ns, attr_local) = xml.resolve_attribute(attr.key);
        if matches!(attr_ns, quick_xml::name::ResolveResult::Bound(n) if oox::is_wordprocessingml(n.as_ref()))
            && attr_local.as_ref() == b"type"
        {
            return Ok(Some(attr.value.to_vec()));
        }
    }
    Ok(None)
}

fn parse_error(message: String) -> docspec_core::Error {
    docspec_core::Error::Parse {
        message,
        position: None,
    }
}

fn start_document_event() -> Event {
    Event::StartDocument {
        id: None,
        language: None,
        metadata: None,
    }
}

fn text_event_from_xml(text: &quick_xml::events::BytesText<'_>) -> Result<Event> {
    let content = text
        .unescape()
        .map_err(|unescape_err| parse_error(format!("XML unescape error: {unescape_err}")))?;
    Ok(Event::Text {
        content: content.into_owned(),
        style: TextStyle::default(),
    })
}

/// Checks whether a resolved namespace and local name match a given `WordprocessingML` element.
///
/// Returns `true` if `ns` resolves to a `WordprocessingML` namespace URI and `local == name`.
#[inline]
fn is_w_element(ns: &quick_xml::name::ResolveResult<'_>, local: &[u8], name: &[u8]) -> bool {
    matches!(ns, quick_xml::name::ResolveResult::Bound(n) if oox::is_wordprocessingml(n.as_ref()))
        && local == name
}

/// Returns paragraph, run, and text matches for a `WordprocessingML` element.
#[inline]
fn w_element_flags(ns: &quick_xml::name::ResolveResult<'_>, local: &[u8]) -> (bool, bool, bool) {
    (
        is_w_paragraph(ns, local),
        is_w_run(ns, local),
        is_w_text(ns, local),
    )
}

/// Returns `true` if the element is a `WordprocessingML` paragraph (`<w:p>`).
#[inline]
fn is_w_paragraph(ns: &quick_xml::name::ResolveResult<'_>, local: &[u8]) -> bool {
    is_w_element(ns, local, b"p")
}

/// Returns `true` if the element is a `WordprocessingML` run (`<w:r>`).
#[inline]
fn is_w_run(ns: &quick_xml::name::ResolveResult<'_>, local: &[u8]) -> bool {
    is_w_element(ns, local, b"r")
}

/// Returns `true` if the element is a `WordprocessingML` text node (`<w:t>`).
#[inline]
fn is_w_text(ns: &quick_xml::name::ResolveResult<'_>, local: &[u8]) -> bool {
    is_w_element(ns, local, b"t")
}

/// Returns `true` if the element is a `WordprocessingML` tab (`<w:tab>`).
#[inline]
fn is_w_tab(ns: &quick_xml::name::ResolveResult<'_>, local: &[u8]) -> bool {
    is_w_element(ns, local, b"tab")
}

/// Returns `true` if the element is a `WordprocessingML` break (`<w:br>`).
#[inline]
fn is_w_br(ns: &quick_xml::name::ResolveResult<'_>, local: &[u8]) -> bool {
    is_w_element(ns, local, b"br")
}

/// Returns `true` if the element is a `WordprocessingML` table (`<w:tbl>`).
#[inline]
fn is_w_table(ns: &quick_xml::name::ResolveResult<'_>, local: &[u8]) -> bool {
    is_w_element(ns, local, b"tbl")
}

/// Returns `true` if the element is a `WordprocessingML` structured document tag (`<w:sdt>`).
#[inline]
fn is_w_sdt(ns: &quick_xml::name::ResolveResult<'_>, local: &[u8]) -> bool {
    is_w_element(ns, local, b"sdt")
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use quick_xml::events::Event as XmlEvent;
    use quick_xml::reader::NsReader;
    use std::io::Write as _;

    #[test]
    fn helpers_identify_w_paragraph_in_transitional_ns() {
        let xml = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p/></w:document>"#;
        let mut reader = NsReader::from_reader(xml.as_ref());
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_resolved_event_into(&mut buf).unwrap() {
                (ns, XmlEvent::Empty(e)) => {
                    let local = e.local_name();
                    assert!(is_w_paragraph(&ns, local.as_ref()));
                    return;
                }
                (_, XmlEvent::Eof) => panic!("no Empty event found"),
                _ => {}
            }
        }
    }

    #[test]
    fn helpers_identify_w_paragraph_in_strict_ns() {
        let xml = br#"<w:document xmlns:w="http://purl.oclc.org/ooxml/wordprocessingml/main"><w:p/></w:document>"#;
        let mut reader = NsReader::from_reader(xml.as_ref());
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_resolved_event_into(&mut buf).unwrap() {
                (ns, XmlEvent::Empty(e)) => {
                    let local = e.local_name();
                    assert!(is_w_paragraph(&ns, local.as_ref()));
                    return;
                }
                (_, XmlEvent::Eof) => panic!("no Empty event found"),
                _ => {}
            }
        }
    }

    #[test]
    fn helpers_identify_w_paragraph_with_alternate_prefix() {
        let xml = br#"<a:document xmlns:a="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><a:p/></a:document>"#;
        let mut reader = NsReader::from_reader(xml.as_ref());
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_resolved_event_into(&mut buf).unwrap() {
                (ns, XmlEvent::Empty(e)) => {
                    let local = e.local_name();
                    assert!(is_w_paragraph(&ns, local.as_ref()));
                    return;
                }
                (_, XmlEvent::Eof) => panic!("no Empty event found"),
                _ => {}
            }
        }
    }

    #[test]
    fn helpers_reject_non_w_element() {
        let xml = br#"<svg:p xmlns:svg="http://www.w3.org/2000/svg"/>"#;
        let mut reader = NsReader::from_reader(xml.as_ref());
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_resolved_event_into(&mut buf).unwrap() {
                (ns, XmlEvent::Empty(e)) => {
                    let local = e.local_name();
                    assert!(!is_w_paragraph(&ns, local.as_ref()));
                    return;
                }
                (_, XmlEvent::Eof) => panic!("no Empty event found"),
                _ => {}
            }
        }
    }

    #[cfg(test)]
    fn build_test_docx(main_part_path: &str, main_xml: &str) -> Vec<u8> {
        use std::io::Write as _;
        use zip::{write::SimpleFileOptions, ZipWriter};
        let rels_xml = format!(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{main_part_path}"/></Relationships>"#
        );
        let buf = std::io::Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);
        let options = SimpleFileOptions::default();
        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(rels_xml.as_bytes()).unwrap();
        zip.start_file(main_part_path, options).unwrap();
        zip.write_all(main_xml.as_bytes()).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[cfg(test)]
    fn drain_events<R: Read + Seek>(reader: &mut DocxReader<R>) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        while let Some(ev) = reader.next_event()? {
            events.push(ev);
        }
        Ok(events)
    }

    #[cfg(test)]
    fn text_contents(events: &[Event]) -> Vec<&str> {
        let mut contents = Vec::new();
        for event in events {
            if let Event::Text { content, .. } = event {
                contents.push(content.as_str());
            }
        }
        contents
    }

    #[cfg(test)]
    fn build_docx_with_body(body_xml: &str) -> Vec<u8> {
        let main_xml = format!(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body_xml}</w:body></w:document>"#
        );
        build_test_docx("word/document.xml", &main_xml)
    }

    #[test]
    fn single_paragraph_with_no_runs_emits_start_then_end() {
        let docx = build_docx_with_body("<w:p></w:p>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn self_closing_paragraph_emits_start_then_end() {
        let docx = build_docx_with_body("<w:p/>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn three_empty_paragraphs_emit_three_pairs() {
        let docx = build_docx_with_body("<w:p/><w:p/><w:p/>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn text_in_paragraph_emits_text_event() {
        let docx = build_docx_with_body("<w:p><w:r><w:t>Hello</w:t></w:r></w:p>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
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
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn paragraph_inside_table_is_suppressed() {
        let body = "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>";
        let docx = build_docx_with_body(body);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
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
    fn paragraph_inside_sdt_is_suppressed() {
        let body = "<w:sdt><w:sdtContent><w:p><w:r><w:t>controlled</w:t></w:r></w:p></w:sdtContent></w:sdt>";
        let docx = build_docx_with_body(body);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
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
    fn paragraph_after_table_emits_normally() {
        let body =
            "<w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl><w:p><w:r><w:t>after</w:t></w:r></w:p>";
        let docx = build_docx_with_body(body);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
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
                    content: "after".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn nested_table_in_table() {
        let body = "<w:tbl><w:tr><w:tc><w:tbl><w:tr><w:tc><w:p><w:r><w:t>deep</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl>";
        let docx = build_docx_with_body(body);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
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
    fn xml_entities_are_unescaped() {
        let docx = build_docx_with_body("<w:p><w:r><w:t>&amp;lt;&amp;amp;</w:t></w:r></w:p>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();
        let text_content = text_contents(&events);

        // &amp;lt; → &lt; (one level of unescaping)
        // &amp;amp; → &amp;
        assert_eq!(text_content, vec!["&lt;&amp;"]);
    }

    #[test]
    fn whitespace_in_text_is_preserved() {
        let docx = build_docx_with_body(
            r#"<w:p><w:r><w:t xml:space="preserve">  hello  </w:t></w:r></w:p>"#,
        );
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();
        let text_content = text_contents(&events);

        assert_eq!(text_content, vec!["  hello  "]);
    }

    #[test]
    fn empty_text_element_emits_empty_text() {
        let docx = build_docx_with_body("<w:p><w:r><w:t></w:t></w:r></w:p>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();
        let text_content = text_contents(&events);

        assert_eq!(text_content, vec![""]);
    }

    #[test]
    fn multiple_text_elements_in_one_run_emit_multiple_text_events() {
        let docx = build_docx_with_body("<w:p><w:r><w:t>foo</w:t><w:t>bar</w:t></w:r></w:p>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();
        let text_contents = text_contents(&events);

        assert_eq!(text_contents, vec!["foo", "bar"]);
    }

    #[test]
    fn text_outside_run_is_not_emitted() {
        let docx = build_docx_with_body("<w:p><w:t>orphan</w:t></w:p>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tab_in_run_emits_tab_text() {
        let docx = build_docx_with_body("<w:p><w:r><w:tab/></w:r></w:p>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
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
                    content: "\t".to_string(),
                    style: TextStyle::default(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn br_in_run_emits_linebreak() {
        let docx = build_docx_with_body("<w:p><w:r><w:br/></w:r></w:p>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::LineBreak,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn br_with_text_wrapping_type_emits_linebreak() {
        let docx = build_docx_with_body(r#"<w:p><w:r><w:br w:type="textWrapping"/></w:r></w:p>"#);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::LineBreak,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn br_with_page_type_is_ignored() {
        let docx = build_docx_with_body(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn br_with_column_type_is_ignored() {
        let docx = build_docx_with_body(r#"<w:p><w:r><w:br w:type="column"/></w:r></w:p>"#);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn br_with_alternate_prefix_resolves_correctly() {
        let docx = build_docx_with_body(
            r#"<a:p xmlns:a="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><a:r><a:br a:type="page"/></a:r></a:p>"#,
        );
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn tab_outside_run_not_emitted() {
        let docx = build_docx_with_body("<w:p><w:tab/></w:p>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn emits_start_then_end_for_empty_body() {
        let main_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#;
        let docx = build_test_docx("word/document.xml", main_xml);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
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
    fn returns_none_after_finished() {
        let main_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#;
        let docx = build_test_docx("word/document.xml", main_xml);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::EndDocument,
            ]
        );
        assert_eq!(reader.next_event().unwrap(), None);
        assert_eq!(reader.next_event().unwrap(), None);
        assert_eq!(reader.next_event().unwrap(), None);
    }

    #[test]
    fn returns_none_after_failed() {
        let docx = build_test_docx("word/document.xml", "<<<not xml>>>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let first = reader.next_event().unwrap();
        assert!(matches!(
            first,
            Some(Event::StartDocument {
                id: None,
                language: None,
                metadata: None,
            })
        ));
        let second = reader.next_event();
        assert!(second.is_err());
        assert_eq!(reader.next_event().unwrap(), None);
        assert_eq!(reader.next_event().unwrap(), None);
    }

    #[test]
    fn unescape_failure_terminates_stream() {
        // &badentity; is an invalid XML entity that quick-xml's unescape() will reject
        let body = "<w:p><w:r><w:t>&badentity;</w:t></w:r></w:p>";
        let docx = build_docx_with_body(body);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();
        // First call: StartDocument
        let first = reader.next_event().unwrap();
        assert!(matches!(first, Some(Event::StartDocument { .. })));
        // Second call: StartParagraph
        let second = reader.next_event().unwrap();
        assert!(matches!(second, Some(Event::StartParagraph { .. })));
        // Third call: Err (unescape failure)
        let third = reader.next_event();
        assert!(third.is_err());
        // Fourth call: Ok(None) — stream is dead
        let fourth = reader.next_event().unwrap();
        assert!(fourth.is_none());
        // Fifth call: Ok(None) — still dead
        let fifth = reader.next_event().unwrap();
        assert!(fifth.is_none());
    }

    #[test]
    fn new_buffers_main_xml_from_canonical_docx() {
        let main_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#;
        let docx_bytes = build_test_docx("word/document.xml", main_xml);
        let cursor = std::io::Cursor::new(docx_bytes);
        let reader = DocxReader::new(cursor).unwrap();
        let buffered = reader.xml_bytes();
        assert_eq!(buffered, main_xml.as_bytes());
    }

    #[test]
    fn new_returns_io_error_on_truncated_zip() {
        let truncated = b"PK\x03\x04truncated";
        let cursor = std::io::Cursor::new(truncated.to_vec());
        let result = DocxReader::<std::io::Cursor<Vec<u8>>>::new(cursor);
        assert!(matches!(result, Err(docspec_core::Error::Other { .. })));
    }

    #[test]
    fn new_returns_parse_error_when_main_part_missing_in_zip() {
        let rels_xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
        let buf = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(rels_xml.as_bytes()).unwrap();
        let docx_bytes = zip.finish().unwrap().into_inner();
        let cursor = std::io::Cursor::new(docx_bytes);
        let result = DocxReader::<std::io::Cursor<Vec<u8>>>::new(cursor);
        assert!(matches!(
            result,
            Err(docspec_core::Error::Parse { message, .. })
                if message.contains("not found in package")
        ));
    }

    #[test]
    fn xml_parse_error_in_main_document_terminates_stream() {
        // Malformed XML in the main document part
        let docx = build_test_docx("word/document.xml", "<<<not xml>>>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        // First call: StartDocument
        let first = reader.next_event().unwrap();
        assert!(matches!(first, Some(Event::StartDocument { .. })));
        // Second call: XML parse error
        let second = reader.next_event();
        assert!(second.is_err());
        // Third call: Ok(None) — stream is dead
        let third = reader.next_event().unwrap();
        assert!(third.is_none());
    }

    #[test]
    fn br_with_invalid_attribute_terminates_stream() {
        // br element with malformed attribute (missing closing quote)
        let body = r#"<w:p><w:r><w:br w:type="page/></w:r></w:p>"#;
        let docx = build_docx_with_body(body);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        // First call: StartDocument
        let first = reader.next_event().unwrap();
        assert!(matches!(first, Some(Event::StartDocument { .. })));
        // Second call: StartParagraph
        let second = reader.next_event().unwrap();
        assert!(matches!(second, Some(Event::StartParagraph { .. })));
        // Third call: XML parse error from attribute parsing
        let third = reader.next_event();
        assert!(third.is_err());
        // Fourth call: Ok(None) — stream is dead
        let fourth = reader.next_event().unwrap();
        assert!(fourth.is_none());
    }

    #[test]
    fn text_with_invalid_xml_entity_terminates_stream() {
        // Text with invalid XML entity reference
        let body = "<w:p><w:r><w:t>&invalid;</w:t></w:r></w:p>";
        let docx = build_docx_with_body(body);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        // First call: StartDocument
        let first = reader.next_event().unwrap();
        assert!(matches!(first, Some(Event::StartDocument { .. })));
        // Second call: StartParagraph
        let second = reader.next_event().unwrap();
        assert!(matches!(second, Some(Event::StartParagraph { .. })));
        // Third call: Unescape error
        let third = reader.next_event();
        assert!(third.is_err());
        // Fourth call: Ok(None) — stream is dead
        let fourth = reader.next_event().unwrap();
        assert!(fourth.is_none());
    }

    #[test]
    fn empty_self_closing_text_element_emits_empty_text() {
        // Self-closing empty text element: <w:t/>
        let docx = build_docx_with_body("<w:p><w:r><w:t/></w:r></w:p>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();
        let text_content = text_contents(&events);

        assert_eq!(text_content, vec![""]);
    }

    #[test]
    fn empty_self_closing_paragraph_emits_empty_text() {
        // Self-closing empty paragraph: <w:p/>
        let docx = build_docx_with_body("<w:p/>");
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn br_with_multiple_attributes_finds_type() {
        // br element with multiple attributes, type is not first
        let docx = build_docx_with_body(
            r#"<w:p><w:r><w:br w:id="1" w:type="page" w:val="test"/></w:r></w:p>"#,
        );
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        // br with type="page" should be ignored
        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn br_with_no_type_attribute_emits_linebreak() {
        // br element without type attribute should emit LineBreak
        let docx = build_docx_with_body(r#"<w:p><w:r><w:br w:id="1"/></w:r></w:p>"#);
        let cursor = std::io::Cursor::new(docx);
        let mut reader = DocxReader::new(cursor).unwrap();

        let events = drain_events(&mut reader).unwrap();

        assert_eq!(
            events,
            vec![
                Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                Event::LineBreak,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }
}
