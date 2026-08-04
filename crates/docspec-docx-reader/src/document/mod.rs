//! DOCX main document part (`document.xml`) streaming event parser.

use core::fmt;
use std::io::{BufReader, Read};
use std::sync::{Arc, Mutex};

use docspec_core::{Error, Event, Result, TableHeaderScope};
use quick_xml::events::BytesStart;

use crate::properties;

mod context;
mod emit;
mod input;
mod media;
mod props;
mod text;

pub use context::DocxData;
use context::PackageContext;
use emit::EmitState;
use input::XmlCursor;
use props::{
    parse_ppr, parse_rpr, parse_tcpr, parse_trpr, resolved_run_styles, ResolvedCellProperties,
    ResolvedParagraphProperties, ResolvedRowProperties, ResolvedRunProperties,
};
use text::{collect_text_content, decode_general_ref, normalize_symbol_text, resolve_sym_char};

const MAX_LIST_LEVEL: u32 = 8;

/// Document processing phase.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// `EndDocument` has been emitted.
    Finished,
    /// `StartDocument` not yet emitted.
    NotStarted,
    /// Processing events between `StartDocument` and `EndDocument`.
    Running,
}

/// The kind of block-level element opened for the current paragraph.
///
/// Resolved by paragraph parsing and used to emit matching block start/end events.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ParagraphBlockKind {
    /// Plain paragraph (default).
    Paragraph,
    /// Heading at the given level.
    Heading { level: u8 },
    /// Block quotation.
    BlockQuote,
    /// Preformatted / code block.
    Preformatted,
    /// Ordered list item at the given nesting depth.
    OrderedListItem {
        num_id: u32,
        ilvl: u32,
        start: Option<u64>,
        style_type: docspec_core::ListStyleType,
    },
    /// Unordered list item at the given nesting depth.
    UnorderedListItem {
        num_id: u32,
        ilvl: u32,
        style_type: docspec_core::ListStyleType,
    },
}

struct ParagraphParseState {
    props: ResolvedParagraphProperties,
    properties_seen: bool,
    paragraph_emitted: bool,
    block_kind: ParagraphBlockKind,
    block_start_emitted: bool,
}

enum HyperlinkParseEnd {
    HyperlinkClosed,
    ParagraphClosed,
    Eof,
}

impl Default for ParagraphParseState {
    fn default() -> Self {
        Self {
            props: ResolvedParagraphProperties::default(),
            properties_seen: false,
            paragraph_emitted: false,
            block_kind: ParagraphBlockKind::Paragraph,
            block_start_emitted: false,
        }
    }
}

/// Streaming parser for the DOCX main document XML part.
pub struct DocumentReader {
    /// Document processing phase.
    phase: Phase,
    /// Pending output events and the state that governs them.
    emit: EmitState,
    /// Deferred parser error returned after already-queued events are drained.
    pending_error: Option<Error>,
    /// Read-only package data (styles, numbering, relationships, archive).
    package: PackageContext,
    /// The XML token pump streaming from the document entry.
    input: XmlCursor,
}

impl fmt::Debug for DocumentReader {
    /// Renders parse state only.
    ///
    /// The queue is reported by length rather than contents: it can hold an
    /// entire subtree's events, so formatting them would make `Debug` itself
    /// expensive. The XML reader and scratch buffer are omitted for the same
    /// reason.
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentReader")
            .field("phase", &self.phase)
            .field("queued", &self.emit.queued())
            .field("pending_error", &self.pending_error)
            .field("package", &self.package)
            .finish_non_exhaustive()
    }
}

impl DocumentReader {
    pub fn from_xml_reader_and_archive(
        xml: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>,
        data: DocxData,
        archive: Arc<Mutex<zip::ZipArchive<Box<dyn crate::package::ReadSeek + 'static>>>>,
        content_types: Arc<crate::content_types::ContentTypes>,
    ) -> Self {
        Self {
            phase: Phase::NotStarted,
            emit: EmitState::default(),
            pending_error: None,
            package: PackageContext::new(data, archive, content_types),
            input: XmlCursor::new(xml),
        }
    }

    #[cfg(test)]
    #[cfg(not(coverage))]
    #[allow(clippy::expect_used, clippy::as_conversions)]
    pub(crate) fn from_xml_reader(
        xml: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>,
        data: DocxData,
    ) -> Self {
        use std::io::Cursor;
        const EMPTY_ZIP: &[u8] = &[
            0x50, 0x4B, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let archive = zip::ZipArchive::new(
            Box::new(Cursor::new(EMPTY_ZIP)) as Box<dyn crate::package::ReadSeek + 'static>
        )
        .expect("minimal empty zip must be valid");
        Self::from_xml_reader_and_archive(
            xml,
            data,
            Arc::new(Mutex::new(archive)),
            Arc::new(crate::content_types::ContentTypes::default()),
        )
    }
}

impl DocumentReader {
    fn emit_deferred_styles_if_needed(
        &mut self,
        props: Option<&ResolvedRunProperties>,
        content_emitted: &mut bool,
    ) {
        if *content_emitted {
            return;
        }
        for kind in resolved_run_styles(props) {
            self.emit.emit_style_if_not_open(kind);
        }
        *content_emitted = true;
    }

    fn close_run_styles(&mut self, props: Option<&ResolvedRunProperties>) {
        self.emit.close_styles(resolved_run_styles(props));
    }

    fn handle_run_child_start(
        &mut self,
        start: &BytesStart<'_>,
        props: &mut Option<ResolvedRunProperties>,
        content_emitted: &mut bool,
    ) -> Result<()> {
        let local_name = start.local_name();
        match local_name.as_ref() {
            b"rPr" if !*content_emitted && props.is_none() => {
                *props = Some(parse_rpr(&mut self.input, &self.package)?);
            }
            b"rPr" => self.consume_current_start(start)?,
            b"t" => {
                let text = collect_text_content(&mut self.input)?;
                let text = normalize_symbol_text(props.as_ref().and_then(|p| p.font), text);
                if !text.is_empty() {
                    self.emit_deferred_styles_if_needed(props.as_ref(), content_emitted);
                    self.emit.push(Event::Text { content: text });
                }
            }
            b"tab" => {
                self.emit_deferred_styles_if_needed(props.as_ref(), content_emitted);
                self.emit.push(Event::Text {
                    content: "\t".to_string(),
                });
                self.input.skip_subtree(start)?;
            }
            b"br" => {
                self.emit_deferred_styles_if_needed(props.as_ref(), content_emitted);
                self.emit.push(Event::LineBreak);
                self.input.skip_subtree(start)?;
            }
            b"drawing" => {
                self.emit_deferred_styles_if_needed(props.as_ref(), content_emitted);
                self.parse_drawing_subtree()?;
            }
            b"pict" => {
                self.emit_deferred_styles_if_needed(props.as_ref(), content_emitted);
                self.parse_pict_subtree()?;
            }
            b"instrText" | b"fldChar" => {
                self.input.skip_subtree(start)?;
            }
            _ if is_denied_container(local_name.as_ref()) => {
                self.input.skip_subtree(start)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_run_child_empty(
        &mut self,
        empty: &BytesStart<'_>,
        props: &mut Option<ResolvedRunProperties>,
        content_emitted: &mut bool,
    ) {
        let local_name = empty.local_name();
        match local_name.as_ref() {
            b"rPr" if !*content_emitted && props.is_none() => {
                *props = Some(ResolvedRunProperties {
                    kinds: Vec::new(),
                    text_color: None,
                    mark: None,
                    font: None,
                });
            }
            b"tab" => {
                self.emit_deferred_styles_if_needed(props.as_ref(), content_emitted);
                self.emit.push(Event::Text {
                    content: "\t".to_string(),
                });
            }
            b"br" => {
                self.emit_deferred_styles_if_needed(props.as_ref(), content_emitted);
                self.emit.push(Event::LineBreak);
            }
            b"sym" => {
                if let Some(text) = resolve_sym_char(props.as_ref().and_then(|p| p.font), empty) {
                    self.emit_deferred_styles_if_needed(props.as_ref(), content_emitted);
                    self.emit.push(Event::Text { content: text });
                }
            }
            _ => {}
        }
    }

    fn parse_r(&mut self, _start: &BytesStart<'_>) -> Result<()> {
        let mut props = None;
        let mut content_emitted = false;

        loop {
            let event = self.input.read_owned()?;
            match event {
                quick_xml::events::Event::Start(start) => {
                    self.handle_run_child_start(&start, &mut props, &mut content_emitted)?;
                }
                quick_xml::events::Event::Empty(empty) => {
                    self.handle_run_child_empty(&empty, &mut props, &mut content_emitted);
                }
                quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"r" => {
                    if content_emitted {
                        self.close_run_styles(props.as_ref());
                    }
                    return Ok(());
                }
                quick_xml::events::Event::GeneralRef(reference) => {
                    let text = decode_general_ref(&reference)?;
                    if !text.is_empty() {
                        self.emit_deferred_styles_if_needed(props.as_ref(), &mut content_emitted);
                        self.emit.push(Event::Text { content: text });
                    }
                }
                quick_xml::events::Event::End(_)
                | quick_xml::events::Event::Text(_)
                | quick_xml::events::Event::CData(_)
                | quick_xml::events::Event::Comment(_)
                | quick_xml::events::Event::Decl(_)
                | quick_xml::events::Event::PI(_)
                | quick_xml::events::Event::DocType(_) => {}
                quick_xml::events::Event::Eof => {
                    if content_emitted {
                        self.close_run_styles(props.as_ref());
                    }
                    return Ok(());
                }
            }
        }
    }

    fn resolve_paragraph_block_kind(
        &mut self,
        props: &ResolvedParagraphProperties,
    ) -> (ParagraphBlockKind, bool) {
        if self.emit.has_pending_preformatted_close() {
            if matches!(
                props.classification.as_ref(),
                Some(crate::styles::StyleClassification::Code)
            ) {
                self.emit.push(Event::LineBreak);
                self.emit.cancel_preformatted_close();
                return (ParagraphBlockKind::Preformatted, true);
            }
            self.emit.flush_pending_preformatted_close();
        }

        let list_classification = props.list_info.and_then(|(num_id, raw_ilvl)| {
            let ilvl = core::cmp::min(raw_ilvl, MAX_LIST_LEVEL);
            let result = self.package.resolve_numbering(num_id, ilvl);
            result
                .is_list
                .then_some((num_id, ilvl, result.is_ordered, result.style_type))
        });

        let block_kind = match props.classification.as_ref() {
            Some(crate::styles::StyleClassification::Heading { level }) => {
                self.emit.flush_list_stack();
                ParagraphBlockKind::Heading { level: *level }
            }
            Some(crate::styles::StyleClassification::BlockQuote) => {
                self.emit.flush_list_stack();
                ParagraphBlockKind::BlockQuote
            }
            Some(crate::styles::StyleClassification::Code) => {
                self.emit.flush_list_stack();
                ParagraphBlockKind::Preformatted
            }
            _ => match list_classification {
                None => ParagraphBlockKind::Paragraph,
                Some((num_id, ilvl, is_ordered, style_type)) => {
                    let sequential = self.emit.reconcile_list_stack(num_id, ilvl, is_ordered);
                    let start = self.emit.compute_start(num_id, ilvl, sequential);
                    if is_ordered {
                        ParagraphBlockKind::OrderedListItem {
                            num_id,
                            ilvl,
                            start,
                            style_type,
                        }
                    } else {
                        ParagraphBlockKind::UnorderedListItem {
                            num_id,
                            ilvl,
                            style_type,
                        }
                    }
                }
            },
        };
        (block_kind, false)
    }

    fn emit_paragraph_start_for_block(
        &mut self,
        props: &ResolvedParagraphProperties,
        block_kind: &ParagraphBlockKind,
    ) {
        match block_kind {
            ParagraphBlockKind::Paragraph => {
                self.emit.flush_list_stack();
                self.emit.push(Event::StartParagraph {
                    alignment: props.alignment.clone(),
                    id: None,
                });
            }
            ParagraphBlockKind::Heading { level } => {
                self.emit.push(Event::StartHeading {
                    level: *level,
                    id: None,
                });
            }
            ParagraphBlockKind::BlockQuote => {
                self.emit.push(Event::StartBlockQuote { id: None });
            }
            ParagraphBlockKind::Preformatted => {
                self.emit.push(Event::StartPreformatted {
                    id: None,
                    syntax: None,
                });
            }
            ParagraphBlockKind::OrderedListItem {
                num_id,
                ilvl,
                start,
                style_type,
            } => {
                self.emit
                    .emit_list_item_start_ordered(*num_id, *ilvl, *start, *style_type);
                self.emit.push(Event::StartParagraph {
                    alignment: props.alignment.clone(),
                    id: None,
                });
            }
            ParagraphBlockKind::UnorderedListItem {
                num_id,
                ilvl,
                style_type,
            } => {
                self.emit
                    .emit_list_item_start_unordered(*num_id, *ilvl, *style_type);
                self.emit.push(Event::StartParagraph {
                    alignment: props.alignment.clone(),
                    id: None,
                });
            }
        }
    }

    fn close_paragraph_block(&mut self, block_kind: &ParagraphBlockKind) {
        self.emit.close_all_styles();

        let end_event = match block_kind {
            ParagraphBlockKind::Paragraph
            | ParagraphBlockKind::OrderedListItem { .. }
            | ParagraphBlockKind::UnorderedListItem { .. } => Event::EndParagraph,
            ParagraphBlockKind::Heading { .. } => Event::EndHeading,
            ParagraphBlockKind::BlockQuote => Event::EndBlockQuote,
            ParagraphBlockKind::Preformatted => {
                self.emit.defer_preformatted_close();
                return;
            }
        };
        self.emit.push(end_event);
    }

    fn parse_empty_p(&mut self) {
        let props = ResolvedParagraphProperties::default();
        let (block_kind, block_start_emitted) = self.resolve_paragraph_block_kind(&props);
        if !block_start_emitted {
            self.emit_paragraph_start_for_block(&props, &block_kind);
        }
        self.close_paragraph_block(&block_kind);
    }

    fn ensure_local_paragraph_started(&mut self, state: &mut ParagraphParseState) {
        if state.paragraph_emitted {
            return;
        }
        if !state.block_start_emitted {
            self.emit_paragraph_start_for_block(&state.props, &state.block_kind);
            state.block_start_emitted = true;
        }
        state.paragraph_emitted = true;
    }

    fn resolve_local_ppr(&mut self, state: &mut ParagraphParseState) {
        let (block_kind, block_start_emitted) = self.resolve_paragraph_block_kind(&state.props);
        state.block_kind = block_kind;
        state.block_start_emitted = block_start_emitted;
    }

    fn emit_local_tab(&mut self, state: &mut ParagraphParseState) {
        self.ensure_local_paragraph_started(state);
        self.emit.push(Event::Text {
            content: "\t".to_string(),
        });
    }

    fn emit_local_line_break(&mut self, state: &mut ParagraphParseState) {
        self.ensure_local_paragraph_started(state);
        self.emit.push(Event::LineBreak);
    }

    fn drain_new_events_into(&mut self, existing_len: usize, buffered: &mut Vec<Event>) {
        self.emit.drain_since(existing_len, buffered);
    }

    fn resolved_hyperlink(
        &self,
        tag: &BytesStart<'_>,
        is_preformatted: bool,
    ) -> Option<(String, Option<String>)> {
        if is_preformatted {
            return None;
        }

        let rid = read_attribute(tag, b"r:id");
        let anchor = read_attribute(tag, b"w:anchor");
        let tooltip = read_attribute(tag, b"w:tooltip");

        let href = if let Some(rid_val) = rid {
            self.package.hyperlink_target(&rid_val)?
        } else {
            let anchor_val = anchor.filter(|a| !a.is_empty())?;
            format!("#{anchor_val}")
        };

        let title = tooltip.and_then(|t| {
            quick_xml::escape::unescape(&t)
                .ok()
                .map(std::borrow::Cow::into_owned)
        });

        Some((href, title))
    }

    fn emit_buffered_hyperlink(
        &mut self,
        resolved: Option<(String, Option<String>)>,
        buffered: Vec<Event>,
    ) {
        if buffered.is_empty() {
            return;
        }

        if let Some((href, title)) = resolved {
            self.emit.push(Event::StartLink {
                href,
                id: None,
                title,
            });
            self.emit.extend(buffered);
            self.emit.push(Event::EndLink);
        } else {
            self.emit.extend(buffered);
        }
    }

    fn parse_hyperlink(
        &mut self,
        start: &BytesStart<'_>,
        is_preformatted: bool,
        is_empty: bool,
    ) -> Result<HyperlinkParseEnd> {
        let resolved = self.resolved_hyperlink(start, is_preformatted);
        let mut buffered: Vec<Event> = Vec::new();

        if is_empty {
            self.emit_buffered_hyperlink(resolved, buffered);
            return Ok(HyperlinkParseEnd::HyperlinkClosed);
        }

        let mut nested_depth: u32 = 1;
        while nested_depth > 0 {
            let event = self.input.read_owned()?;
            match event {
                quick_xml::events::Event::Start(tag) => match tag.local_name().as_ref() {
                    b"r" => {
                        let existing_len = self.emit.queued();
                        self.parse_r(&tag)?;
                        self.drain_new_events_into(existing_len, &mut buffered);
                    }
                    b"hyperlink" => {
                        nested_depth = nested_depth.saturating_add(1);
                    }
                    _ => {
                        self.input.skip_subtree(&tag)?;
                    }
                },
                quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"hyperlink" => {
                    nested_depth = nested_depth.saturating_sub(1);
                }
                quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"p" => {
                    self.emit_buffered_hyperlink(resolved, buffered);
                    return Ok(HyperlinkParseEnd::ParagraphClosed);
                }
                quick_xml::events::Event::Empty(_)
                | quick_xml::events::Event::End(_)
                | quick_xml::events::Event::Text(_)
                | quick_xml::events::Event::GeneralRef(_)
                | quick_xml::events::Event::CData(_)
                | quick_xml::events::Event::Comment(_)
                | quick_xml::events::Event::Decl(_)
                | quick_xml::events::Event::PI(_)
                | quick_xml::events::Event::DocType(_) => {}
                quick_xml::events::Event::Eof => {
                    self.emit_buffered_hyperlink(resolved, buffered);
                    return Ok(HyperlinkParseEnd::Eof);
                }
            }
        }

        self.emit_buffered_hyperlink(resolved, buffered);
        Ok(HyperlinkParseEnd::HyperlinkClosed)
    }

    fn consume_current_start(&mut self, start: &BytesStart<'_>) -> Result<()> {
        self.input.skip_subtree(start)?;
        Ok(())
    }

    fn handle_paragraph_start_child(
        &mut self,
        start: &BytesStart<'_>,
        state: &mut ParagraphParseState,
    ) -> Result<bool> {
        match start.local_name().as_ref() {
            b"pPr" if !state.paragraph_emitted && !state.properties_seen => {
                state.props = parse_ppr(&mut self.input, &self.package)?;
                state.properties_seen = true;
                self.resolve_local_ppr(state);
            }
            b"pPr" => self.consume_current_start(start)?,
            b"tab" => {
                self.emit_local_tab(state);
                self.consume_current_start(start)?;
            }
            b"br" => {
                self.emit_local_line_break(state);
                self.consume_current_start(start)?;
            }
            b"r" => {
                self.ensure_local_paragraph_started(state);
                self.parse_r(start)?;
            }
            b"hyperlink" => {
                self.ensure_local_paragraph_started(state);
                match self.parse_hyperlink(
                    start,
                    matches!(state.block_kind, ParagraphBlockKind::Preformatted),
                    false,
                )? {
                    HyperlinkParseEnd::HyperlinkClosed => {}
                    HyperlinkParseEnd::ParagraphClosed => {
                        self.finish_paragraph_parse(state);
                        return Ok(true);
                    }
                    HyperlinkParseEnd::Eof => {
                        self.finish_paragraph_parse(state);
                        self.handle_eof();
                        return Ok(true);
                    }
                }
            }
            b"drawing" => {
                self.ensure_local_paragraph_started(state);
                self.parse_drawing_subtree()?;
            }
            b"pict" => {
                self.ensure_local_paragraph_started(state);
                self.parse_pict_subtree()?;
            }
            local if is_denied_container(local) => self.consume_current_start(start)?,
            _ => {}
        }
        Ok(false)
    }

    fn handle_paragraph_empty_child(
        &mut self,
        empty: &BytesStart<'_>,
        state: &mut ParagraphParseState,
    ) -> Result<()> {
        match empty.local_name().as_ref() {
            b"pPr" if !state.paragraph_emitted && !state.properties_seen => {
                state.props = ResolvedParagraphProperties::default();
                state.properties_seen = true;
                self.resolve_local_ppr(state);
            }
            b"tab" => self.emit_local_tab(state),
            b"br" => self.emit_local_line_break(state),
            b"r" | b"drawing" | b"pict" => self.ensure_local_paragraph_started(state),
            b"hyperlink" => {
                self.ensure_local_paragraph_started(state);
                self.parse_hyperlink(
                    empty,
                    matches!(state.block_kind, ParagraphBlockKind::Preformatted),
                    true,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn finish_paragraph_parse(&mut self, state: &mut ParagraphParseState) {
        self.ensure_local_paragraph_started(state);
        self.close_paragraph_block(&state.block_kind);
    }

    fn parse_p(&mut self, _start: &BytesStart<'_>) -> Result<()> {
        let mut state = ParagraphParseState::default();
        loop {
            let event = self.input.read_owned()?;

            match event {
                quick_xml::events::Event::Start(start) => {
                    if self.handle_paragraph_start_child(&start, &mut state)? {
                        return Ok(());
                    }
                }
                quick_xml::events::Event::Empty(empty) => {
                    self.handle_paragraph_empty_child(&empty, &mut state)?;
                }
                quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"p" => {
                    self.finish_paragraph_parse(&mut state);
                    return Ok(());
                }
                quick_xml::events::Event::End(_)
                | quick_xml::events::Event::Text(_)
                | quick_xml::events::Event::GeneralRef(_)
                | quick_xml::events::Event::CData(_)
                | quick_xml::events::Event::Comment(_)
                | quick_xml::events::Event::Decl(_)
                | quick_xml::events::Event::PI(_)
                | quick_xml::events::Event::DocType(_) => {}
                quick_xml::events::Event::Eof => {
                    self.finish_paragraph_parse(&mut state);
                    self.handle_eof();
                    return Ok(());
                }
            }
        }
    }

    fn emit_table_cell_start(&mut self, is_header: bool, colspan: Option<u32>) {
        if is_header {
            self.emit.push(Event::StartTableHeader {
                scope: Some(TableHeaderScope::Column),
                abbr: None,
                colspan,
                rowspan: None,
                id: None,
            });
        } else {
            self.emit.push(Event::StartTableCell {
                colspan,
                rowspan: None,
                id: None,
            });
        }
    }

    fn emit_empty_table_cell(&mut self, is_header: bool, colspan: Option<u32>) {
        self.emit_table_cell_start(is_header, colspan);
        if is_header {
            self.emit.push(Event::EndTableHeader);
        } else {
            self.emit.push(Event::EndTableCell);
        }
    }

    fn parse_tc(&mut self, _start: &BytesStart<'_>, is_header: bool) -> Result<()> {
        let mut cell_props: Option<ResolvedCellProperties> = None;
        let mut cell_started = false;
        loop {
            let event = self.input.read_owned()?;

            match event {
                quick_xml::events::Event::Start(start) => match start.local_name().as_ref() {
                    b"tcPr" if !cell_started && cell_props.is_none() => {
                        cell_props = Some(parse_tcpr(&mut self.input)?);
                    }
                    b"tcPr" => {
                        self.input.skip_subtree(&start)?;
                    }
                    b"p" => {
                        if !cell_started {
                            let colspan = cell_props.as_ref().and_then(|props| props.colspan);
                            self.emit_table_cell_start(is_header, colspan);
                            cell_started = true;
                        }
                        self.parse_p(&start)?;
                    }
                    b"tbl" => {
                        if !cell_started {
                            let colspan = cell_props.as_ref().and_then(|props| props.colspan);
                            self.emit_table_cell_start(is_header, colspan);
                            cell_started = true;
                        }
                        self.parse_tbl(&start, false)?;
                    }
                    _ if is_denied_container(start.local_name().as_ref()) => {
                        self.input.skip_subtree(&start)?;
                    }
                    _ => {}
                },
                quick_xml::events::Event::Empty(empty) => match empty.local_name().as_ref() {
                    b"tcPr" if !cell_started && cell_props.is_none() => {
                        cell_props = Some(ResolvedCellProperties::default());
                    }
                    b"p" => {
                        if !cell_started {
                            let colspan = cell_props.as_ref().and_then(|props| props.colspan);
                            self.emit_table_cell_start(is_header, colspan);
                            cell_started = true;
                        }
                        self.emit.push(Event::StartParagraph {
                            alignment: None,
                            id: None,
                        });
                        self.emit.push(Event::EndParagraph);
                    }
                    _ => {}
                },
                quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"tc" => {
                    self.emit.flush_list_stack();
                    self.emit.flush_pending_preformatted_close();
                    if !cell_started {
                        let colspan = cell_props.as_ref().and_then(|props| props.colspan);
                        self.emit_empty_table_cell(is_header, colspan);
                    } else if is_header {
                        self.emit.push(Event::EndTableHeader);
                    } else {
                        self.emit.push(Event::EndTableCell);
                    }
                    return Ok(());
                }
                quick_xml::events::Event::Eof => {
                    return Err(parse_error(
                        "malformed document.xml: unexpected EOF inside <w:tc>".to_string(),
                    ));
                }
                quick_xml::events::Event::End(_)
                | quick_xml::events::Event::Text(_)
                | quick_xml::events::Event::GeneralRef(_)
                | quick_xml::events::Event::CData(_)
                | quick_xml::events::Event::Comment(_)
                | quick_xml::events::Event::Decl(_)
                | quick_xml::events::Event::PI(_)
                | quick_xml::events::Event::DocType(_) => {}
            }
        }
    }

    fn parse_tr(&mut self, _start: &BytesStart<'_>, header_band_active: &mut bool) -> Result<()> {
        let mut row_props: Option<ResolvedRowProperties> = None;
        let mut row_started = false;
        loop {
            let event = self.input.read_owned()?;

            match event {
                quick_xml::events::Event::Start(start) => match start.local_name().as_ref() {
                    b"trPr" if !row_started && row_props.is_none() => {
                        row_props = Some(parse_trpr(&mut self.input)?);
                        if row_props.as_ref().is_some_and(|props| !props.is_header) {
                            *header_band_active = false;
                        }
                    }
                    b"trPr" => {
                        self.input.skip_subtree(&start)?;
                    }
                    b"tc" => {
                        let row_is_header = row_props.as_ref().is_some_and(|props| props.is_header);
                        if !row_started {
                            if !row_is_header {
                                *header_band_active = false;
                            }
                            self.emit.push(Event::StartTableRow { id: None });
                            row_started = true;
                        }
                        self.parse_tc(&start, row_is_header && *header_band_active)?;
                    }
                    _ if is_denied_container(start.local_name().as_ref()) => {
                        self.input.skip_subtree(&start)?;
                    }
                    _ => {}
                },
                quick_xml::events::Event::Empty(empty) => match empty.local_name().as_ref() {
                    b"trPr" if !row_started && row_props.is_none() => {
                        row_props = Some(ResolvedRowProperties::default());
                        *header_band_active = false;
                    }
                    b"tc" => {
                        let row_is_header = row_props.as_ref().is_some_and(|props| props.is_header);
                        if !row_started {
                            if !row_is_header {
                                *header_band_active = false;
                            }
                            self.emit.push(Event::StartTableRow { id: None });
                            row_started = true;
                        }
                        self.emit_empty_table_cell(row_is_header && *header_band_active, None);
                    }
                    _ => {}
                },
                quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"tr" => {
                    self.emit.flush_pending_preformatted_close();
                    if row_started {
                        self.emit.push(Event::EndTableRow);
                    }
                    return Ok(());
                }
                quick_xml::events::Event::Eof => {
                    return Err(parse_error(
                        "malformed document.xml: unexpected EOF inside <w:tr>".to_string(),
                    ));
                }
                quick_xml::events::Event::End(_)
                | quick_xml::events::Event::Text(_)
                | quick_xml::events::Event::GeneralRef(_)
                | quick_xml::events::Event::CData(_)
                | quick_xml::events::Event::Comment(_)
                | quick_xml::events::Event::Decl(_)
                | quick_xml::events::Event::PI(_)
                | quick_xml::events::Event::DocType(_) => {}
            }
        }
    }

    fn parse_tbl(&mut self, _start: &BytesStart<'_>, is_outermost: bool) -> Result<()> {
        self.emit.flush_list_stack();
        self.emit.flush_pending_preformatted_close();
        self.emit.push(Event::StartTable { id: None });
        let mut header_band_active = is_outermost;
        loop {
            let event = self.input.read_owned()?;

            match event {
                quick_xml::events::Event::Start(start) => {
                    if start.local_name().as_ref() == b"tr" {
                        self.parse_tr(&start, &mut header_band_active)?;
                    }
                    if is_denied_container(start.local_name().as_ref()) {
                        self.input.skip_subtree(&start)?;
                    }
                }
                quick_xml::events::Event::Empty(empty) => {
                    if empty.local_name().as_ref() == b"tr" {
                        header_band_active = false;
                        self.emit.push(Event::StartTableRow { id: None });
                        self.emit.push(Event::EndTableRow);
                    }
                }
                quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"tbl" => {
                    self.emit.flush_list_stack();
                    self.emit.flush_pending_preformatted_close();
                    self.emit.push(Event::EndTable);
                    return Ok(());
                }
                quick_xml::events::Event::Eof => {
                    return Err(parse_error(
                        "malformed document.xml: unexpected EOF inside <w:tbl>".to_string(),
                    ));
                }
                quick_xml::events::Event::End(_)
                | quick_xml::events::Event::Text(_)
                | quick_xml::events::Event::GeneralRef(_)
                | quick_xml::events::Event::CData(_)
                | quick_xml::events::Event::Comment(_)
                | quick_xml::events::Event::Decl(_)
                | quick_xml::events::Event::PI(_)
                | quick_xml::events::Event::DocType(_) => {}
            }
        }
    }

    fn handle_eof(&mut self) {
        self.emit.flush_list_stack();
        self.emit.flush_pending_preformatted_close();
        self.emit.push(Event::EndDocument);
        self.phase = Phase::Finished;
    }

    fn handle_start(&mut self, tag: &BytesStart<'_>) -> Result<()> {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        if local == b"drawing" {
            self.parse_drawing_subtree()?;
            return Ok(());
        }
        if local == b"pict" {
            self.parse_pict_subtree()?;
            return Ok(());
        }
        if local == b"tbl" {
            self.parse_tbl(tag, true)?;
            return Ok(());
        }
        match local {
            _ if is_denied_container(local) => {
                self.input.skip_subtree(tag)?;
            }
            b"p" => {
                self.parse_p(tag)?;
                return Ok(());
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_empty_body_tag(&mut self, tag: &BytesStart<'_>) {
        if tag.local_name().as_ref() == b"p" {
            self.parse_empty_p();
        }
    }

    fn read_until_event(&mut self) -> Result<()> {
        let event = self.input.read_owned()?;

        match event {
            quick_xml::events::Event::Start(tag) => self.handle_start(&tag)?,
            quick_xml::events::Event::Empty(tag) => self.handle_empty_body_tag(&tag),
            quick_xml::events::Event::End(_)
            | quick_xml::events::Event::Text(_)
            | quick_xml::events::Event::GeneralRef(_)
            | quick_xml::events::Event::CData(_)
            | quick_xml::events::Event::Comment(_)
            | quick_xml::events::Event::Decl(_)
            | quick_xml::events::Event::PI(_)
            | quick_xml::events::Event::DocType(_) => {}
            quick_xml::events::Event::Eof => self.handle_eof(),
        }

        Ok(())
    }

    /// Returns the next parsed `DocSpec` event from `document.xml`.
    #[inline]
    pub fn next_event(&mut self) -> Result<Option<Event>> {
        loop {
            if let Some(event) = self.emit.pop() {
                return Ok(Some(event));
            }
            if let Some(err) = self.pending_error.take() {
                return Err(err);
            }

            match self.phase {
                Phase::NotStarted => {
                    self.phase = Phase::Running;
                    self.emit.push(Event::StartDocument {
                        id: None,
                        language: None,
                        metadata: None,
                    });
                }
                Phase::Finished => return Ok(None),
                Phase::Running => {
                    if let Err(err) = self.read_until_event() {
                        if self.emit.is_empty() {
                            return Err(err);
                        }
                        self.pending_error = Some(err);
                    }
                }
            }
        }
    }
}

/// Returns true for elements whose entire subtree should be silently dropped
/// when they appear at the document level.
fn is_denied_container(local: &[u8]) -> bool {
    matches!(
        local,
        b"Fallback"
            | b"object"
            | b"del"
            | b"moveFrom"
            | b"tblPr"
            | b"tblGrid"
            | b"tblPrEx"
            | b"sdtPr"
            | b"sdtEndPr"
    )
}

fn read_val_attribute(tag: &BytesStart<'_>) -> Option<String> {
    let a = tag.try_get_attribute(b"w:val").ok().flatten()?;
    core::str::from_utf8(a.value.as_ref())
        .ok()
        .map(str::to_owned)
}

fn read_attribute(tag: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    let a = tag.try_get_attribute(name).ok().flatten()?;
    core::str::from_utf8(a.value.as_ref())
        .ok()
        .map(str::to_owned)
}

fn read_decoded_attribute(tag: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    let value = read_attribute(tag, name)?;
    quick_xml::escape::unescape(&value)
        .ok()
        .map(std::borrow::Cow::into_owned)
}

fn parse_u32_attr(tag: &BytesStart<'_>, name: &[u8]) -> Option<u32> {
    read_attribute(tag, name)?.parse::<u32>().ok()
}

fn read_rfonts_symbol(tag: &BytesStart<'_>) -> Option<crate::symbol_fonts::SymbolFont> {
    for attr_name in [b"w:ascii".as_ref(), b"w:hAnsi".as_ref(), b"w:cs".as_ref()] {
        if let Some(name) = read_attribute(tag, attr_name) {
            if let Some(font) = crate::symbol_fonts::SymbolFont::from_name(&name) {
                return Some(font);
            }
        }
    }
    None
}

fn parse_on_off_attribute(tag: &BytesStart<'_>) -> bool {
    let val = read_val_attribute(tag);
    properties::parse_on_off(val.as_deref())
}

fn parse_error(message: String) -> Error {
    Error::Parse {
        message,
        position: None,
    }
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::panic,
        clippy::separated_literal_suffix,
        clippy::too_many_lines
    )]
    use std::io::{Cursor, Read};
    use std::sync::Arc;

    use docspec_core::{
        AssetHandle, Color, ImageSource, ListStyleType, TextAlignment, TextStyleKind,
    };

    use super::props::parse_numpr;
    use super::*;
    use crate::rels::HyperlinkMap;

    fn asset_source(id: &str) -> ImageSource {
        #[derive(Debug)]
        struct StubHandle(String);
        impl AssetHandle for StubHandle {
            fn asset_id(&self) -> &str {
                &self.0
            }
            fn content_type(&self) -> Option<std::borrow::Cow<'_, str>> {
                None
            }
            fn stream_to(&self, _: &mut dyn std::io::Write) -> std::io::Result<u64> {
                Ok(0)
            }
        }
        ImageSource::Asset(Arc::new(StubHandle(id.to_string())))
    }

    fn styles_xml(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
{body}
</w:styles>"#
        )
    }

    fn make_docx_data(styles_body: &str) -> DocxData {
        let xml = styles_xml(styles_body);
        let style_list = crate::styles::StyleList::parse(std::io::Cursor::new(xml.into_bytes()))
            .expect("valid styles XML");
        DocxData {
            style_list,
            hyperlink_map: HyperlinkMap::default(),
            numbering: crate::numbering::MinimalNumbering::new(),
            image_map: crate::rels::ImageMap::default(),
        }
    }

    fn numbering_xml(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
{body}
</w:numbering>"#
        )
    }

    fn decimal_numbering() -> crate::numbering::MinimalNumbering {
        let xml = numbering_xml(
            r#"<w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="decimal"/></w:lvl>
</w:abstractNum>
<w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>"#,
        );
        crate::numbering::parse_numbering(Cursor::new(xml.into_bytes()))
            .expect("valid numbering XML")
    }

    fn two_decimal_numbering() -> crate::numbering::MinimalNumbering {
        let xml = numbering_xml(
            r#"<w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="decimal"/></w:lvl>
</w:abstractNum>
<w:abstractNum w:abstractNumId="2">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="decimal"/></w:lvl>
</w:abstractNum>
<w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
<w:num w:numId="2"><w:abstractNumId w:val="2"/></w:num>"#,
        );
        crate::numbering::parse_numbering(Cursor::new(xml.into_bytes()))
            .expect("valid numbering XML")
    }

    fn make_reader_with_numbering(
        document_xml: &str,
        numbering: crate::numbering::MinimalNumbering,
    ) -> DocumentReader {
        let stream: Box<dyn Read + Send> = Box::new(Cursor::new(document_xml.as_bytes().to_vec()));
        let xml = quick_xml::Reader::from_reader(std::io::BufReader::new(stream));
        let data = DocxData {
            style_list: crate::styles::StyleList::default(),
            hyperlink_map: HyperlinkMap::default(),
            numbering,
            image_map: crate::rels::ImageMap::default(),
        };
        DocumentReader::from_xml_reader(xml, data)
    }

    fn list_paragraph(num_id: u32, ilvl: u32, text: &str) -> String {
        format!(
            r#"<w:p><w:pPr><w:numPr><w:numId w:val="{num_id}"/><w:ilvl w:val="{ilvl}"/></w:numPr></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#
        )
    }

    fn plain_paragraph(text: &str) -> String {
        format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>")
    }

    fn document_with_body(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>{body}</w:body>
</w:document>"#
        )
    }

    fn make_reader_with_styles(document_xml: &str, styles_body: &str) -> DocumentReader {
        let stream: Box<dyn std::io::Read + Send> =
            Box::new(std::io::Cursor::new(document_xml.to_string().into_bytes()));
        let xml = quick_xml::Reader::from_reader(std::io::BufReader::new(stream));
        DocumentReader::from_xml_reader(xml, make_docx_data(styles_body))
    }

    fn collect_events(reader: &mut DocumentReader) -> Vec<docspec_core::Event> {
        let mut events = Vec::new();
        loop {
            match reader.next_event() {
                Ok(Some(event)) => {
                    if matches!(event, docspec_core::Event::EndDocument) {
                        events.push(event);
                        break;
                    }
                    events.push(event);
                }
                Ok(None) => break,
                Err(err) => panic!("unexpected error: {err:?}"),
            }
        }
        events
    }

    fn make_reader(document_xml: &str) -> DocumentReader {
        let stream: Box<dyn Read + Send> = Box::new(Cursor::new(document_xml.as_bytes().to_vec()));
        let xml = quick_xml::Reader::from_reader(std::io::BufReader::new(stream));
        let data = DocxData {
            style_list: crate::styles::StyleList::default(),
            hyperlink_map: HyperlinkMap::default(),
            numbering: crate::numbering::MinimalNumbering::new(),
            image_map: crate::rels::ImageMap::default(),
        };
        DocumentReader::from_xml_reader(xml, data)
    }

    fn make_reader_with_hyperlinks(
        document_xml: &str,
        hyperlink_map: HyperlinkMap,
    ) -> DocumentReader {
        let stream: Box<dyn Read + Send> = Box::new(Cursor::new(document_xml.as_bytes().to_vec()));
        let xml = quick_xml::Reader::from_reader(std::io::BufReader::new(stream));
        let data = DocxData {
            style_list: crate::styles::StyleList::default(),
            hyperlink_map,
            numbering: crate::numbering::MinimalNumbering::new(),
            image_map: crate::rels::ImageMap::default(),
        };
        DocumentReader::from_xml_reader(xml, data)
    }

    fn document_with_hyperlink_body(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
    xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>{body}</w:body>
</w:document>"#
        )
    }

    fn read_first_start(reader: &mut DocumentReader) -> BytesStart<'static> {
        loop {
            match reader.input.read_owned().expect("valid XML event") {
                quick_xml::events::Event::Start(start) => return start,
                quick_xml::events::Event::Eof => panic!("expected start tag"),
                _ => {}
            }
        }
    }

    fn make_reader_with_images(
        document_xml: &str,
        image_map: crate::rels::ImageMap,
    ) -> DocumentReader {
        let stream: Box<dyn Read + Send> = Box::new(Cursor::new(document_xml.as_bytes().to_vec()));
        let xml = quick_xml::Reader::from_reader(std::io::BufReader::new(stream));
        let data = DocxData {
            style_list: crate::styles::StyleList::default(),
            hyperlink_map: HyperlinkMap::default(),
            numbering: crate::numbering::MinimalNumbering::new(),
            image_map,
        };
        DocumentReader::from_xml_reader(xml, data)
    }

    #[test]
    fn parse_numpr_with_both() {
        let mut reader = make_reader(
            r#"<w:numPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:numId w:val="7"/><w:ilvl w:val="2"/></w:numPr>"#,
        );
        let _ = read_first_start(&mut reader);
        let parsed = parse_numpr(&mut reader.input).expect("numPr parses");
        assert_eq!(parsed, Some((7, 2)));
    }

    #[test]
    fn parse_numpr_no_ilvl_defaults_to_zero() {
        let mut reader = make_reader(
            r#"<w:numPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:numId w:val="7"/></w:numPr>"#,
        );
        let _ = read_first_start(&mut reader);
        let parsed = parse_numpr(&mut reader.input).expect("numPr parses");
        assert_eq!(parsed, Some((7, 0)));
    }

    #[test]
    fn parse_numpr_unknown_children() {
        let mut reader = make_reader(
            r#"<w:numPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:unknown><w:numId w:val="9"/></w:unknown><w:numId w:val="3"/><w:ilvl w:val="1"/></w:numPr>"#,
        );
        let _ = read_first_start(&mut reader);
        let parsed = parse_numpr(&mut reader.input).expect("numPr parses");
        assert_eq!(parsed, Some((3, 1)));
    }

    #[test]
    fn parse_ppr_all_three() {
        let styles = r#"<w:style w:type="paragraph" w:styleId="Heading2">
    <w:name w:val="heading 2"/>
</w:style>"#;
        let mut reader = make_reader_with_styles(
            r#"<w:pPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:jc w:val="center"/><w:pStyle w:val="Heading2"/><w:numPr><w:numId w:val="4"/><w:ilvl w:val="1"/></w:numPr></w:pPr>"#,
            styles,
        );
        let _ = read_first_start(&mut reader);
        let parsed = parse_ppr(&mut reader.input, &reader.package).expect("pPr parses");
        assert_eq!(
            parsed,
            ResolvedParagraphProperties {
                alignment: Some(TextAlignment::Center),
                classification: Some(crate::styles::StyleClassification::Heading { level: 2 }),
                list_info: Some((4, 1)),
            }
        );
    }

    #[test]
    fn parse_ppr_empty_returns_default() {
        let mut reader = make_reader(
            r#"<w:pPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:pPr>"#,
        );
        let _ = read_first_start(&mut reader);
        let parsed = parse_ppr(&mut reader.input, &reader.package).expect("pPr parses");
        assert_eq!(parsed, ResolvedParagraphProperties::default());
    }

    #[test]
    fn parse_p_empty() {
        let doc = document_with_body("<w:p></w:p>");
        let mut reader = make_reader(&doc);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument
            ]
        );
    }

    #[test]
    fn parse_p_one_styled_run() {
        let doc = document_with_body("<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r></w:p>");
        let mut reader = make_reader(&doc);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                Event::Text {
                    content: "bold".to_string(),
                },
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn b_cs_alone_does_not_emit_bold() {
        let doc =
            document_with_body("<w:p><w:r><w:rPr><w:bCs/></w:rPr><w:t>text</w:t></w:r></w:p>");
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::Text {
                    content: "text".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn i_cs_alone_does_not_emit_italic() {
        let doc =
            document_with_body("<w:p><w:r><w:rPr><w:iCs/></w:rPr><w:t>text</w:t></w:r></w:p>");
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::Text {
                    content: "text".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn b_with_b_cs_still_emits_bold_once() {
        let doc = document_with_body(
            "<w:p><w:r><w:rPr><w:b/><w:bCs/></w:rPr><w:t>bold</w:t></w:r></w:p>",
        );
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                Event::Text {
                    content: "bold".to_string(),
                },
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn b_with_b_cs_off_still_emits_bold() {
        let doc = document_with_body(
            r#"<w:p><w:r><w:rPr><w:b/><w:bCs w:val="false"/></w:rPr><w:t>bold</w:t></w:r></w:p>"#,
        );
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                Event::Text {
                    content: "bold".to_string(),
                },
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn i_with_i_cs_still_emits_italic_once() {
        let doc = document_with_body(
            "<w:p><w:r><w:rPr><w:i/><w:iCs/></w:rPr><w:t>italic</w:t></w:r></w:p>",
        );
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Italic,
                    id: None,
                },
                Event::Text {
                    content: "italic".to_string(),
                },
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn parse_p_second_ppr_is_ignored() {
        let doc = document_with_body(
            r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>text</w:t></w:r></w:p>"#,
        );
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                Event::StartParagraph {
                    alignment: Some(TextAlignment::Center),
                    id: None,
                },
                Event::Text {
                    content: "text".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn parse_p_heading_via_pstyle() {
        let styles = r#"<w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
</w:style>"#;
        let doc = document_with_body(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>title</w:t></w:r></w:p>"#,
        );
        let mut reader = make_reader_with_styles(&doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                start_doc(),
                Event::StartHeading { level: 1, id: None },
                Event::Text {
                    content: "title".to_string(),
                },
                Event::EndHeading,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn parse_hyperlink_with_text() {
        let mut hyperlink_map = HyperlinkMap::default();
        hyperlink_map.insert("rId1".to_string(), "https://example.com".to_string());
        let doc = document_with_hyperlink_body(
            r#"<w:p><w:hyperlink r:id="rId1" w:tooltip="tip &amp; title"><w:r><w:t>link</w:t></w:r></w:hyperlink></w:p>"#,
        );
        let mut reader = make_reader_with_hyperlinks(&doc, hyperlink_map);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::StartLink {
                    href: "https://example.com".to_string(),
                    id: None,
                    title: Some("tip & title".to_string()),
                },
                Event::Text {
                    content: "link".to_string(),
                },
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn parse_hyperlink_empty_emits_nothing() {
        let mut hyperlink_map = HyperlinkMap::default();
        hyperlink_map.insert("rId1".to_string(), "https://example.com".to_string());
        let doc = document_with_hyperlink_body(r#"<w:p><w:hyperlink r:id="rId1"/></w:p>"#);
        let mut reader = make_reader_with_hyperlinks(&doc, hyperlink_map);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn parse_hyperlink_nested_flattens() {
        let mut hyperlink_map = HyperlinkMap::default();
        hyperlink_map.insert("rId1".to_string(), "https://outer.example".to_string());
        hyperlink_map.insert("rId2".to_string(), "https://inner.example".to_string());
        let doc = document_with_hyperlink_body(
            r#"<w:p><w:hyperlink r:id="rId1"><w:r><w:t>outer</w:t></w:r><w:hyperlink r:id="rId2"><w:r><w:t>inner</w:t></w:r></w:hyperlink></w:hyperlink></w:p>"#,
        );
        let mut reader = make_reader_with_hyperlinks(&doc, hyperlink_map);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::StartLink {
                    href: "https://outer.example".to_string(),
                    id: None,
                    title: None,
                },
                Event::Text {
                    content: "outer".to_string(),
                },
                Event::Text {
                    content: "inner".to_string(),
                },
                Event::EndLink,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    fn image_rel(target: &str, is_external: bool) -> crate::rels::ImageRel {
        crate::rels::ImageRel {
            target: target.to_string(),
            is_external,
        }
    }

    fn image_event(source: ImageSource, alt: Option<&str>) -> Event {
        Event::Image {
            alt: alt.map(str::to_string),
            decorative: false,
            id: None,
            source,
            title: None,
        }
    }

    fn drawing_document(drawing_inner: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
    xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
    xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
    xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">
  <w:body><w:p><w:r><w:drawing>{drawing_inner}</w:drawing></w:r></w:p></w:body>
</w:document>"#
        )
    }

    fn picture_with_blip(doc_pr_attrs: &str, blip: &str) -> String {
        format!(
            "<wp:inline>
  <wp:docPr {doc_pr_attrs}/>
  <a:graphic><a:graphicData><pic:pic><pic:blipFill>{blip}</pic:blipFill></pic:pic></a:graphicData></a:graphic>
</wp:inline>"
        )
    }

    fn start_doc() -> Event {
        Event::StartDocument {
            id: None,
            language: None,
            metadata: None,
        }
    }

    fn start_para() -> Event {
        Event::StartParagraph {
            alignment: None,
            id: None,
        }
    }

    fn start_cell(colspan: Option<u32>) -> Event {
        Event::StartTableCell {
            colspan,
            rowspan: None,
            id: None,
        }
    }

    fn start_header(colspan: Option<u32>) -> Event {
        Event::StartTableHeader {
            abbr: None,
            colspan,
            id: None,
            rowspan: None,
            scope: Some(TableHeaderScope::Column),
        }
    }

    fn drain_queue(reader: &mut DocumentReader) -> Vec<Event> {
        core::iter::from_fn(|| reader.emit.pop()).collect()
    }

    fn parse_rpr_fragment(xml_fragment: &str) -> ResolvedRunProperties {
        let mut reader = make_reader(xml_fragment);
        loop {
            match reader.input.read_owned() {
                Ok(quick_xml::events::Event::Start(start))
                    if start.local_name().as_ref() == b"rPr" =>
                {
                    return parse_rpr(&mut reader.input, &reader.package)
                        .expect("rPr fragment parses");
                }
                Ok(quick_xml::events::Event::Eof) => panic!("missing rPr start"),
                Ok(_) => {}
                Err(err) => panic!("unexpected XML error: {err}"),
            }
        }
    }

    #[test]
    fn parse_tcpr_gridspan() {
        let mut reader = make_reader(
            r#"<w:tcPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:gridSpan w:val="3"/></w:tcPr>"#,
        );
        let _ = read_first_start(&mut reader);
        let parsed = parse_tcpr(&mut reader.input).expect("tcPr parses");

        assert_eq!(parsed, ResolvedCellProperties { colspan: Some(3) });
    }

    #[test]
    fn parse_tcpr_empty_returns_none_colspan() {
        let mut reader = make_reader(
            r#"<w:tcPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:tcPr>"#,
        );
        let _ = read_first_start(&mut reader);
        let parsed = parse_tcpr(&mut reader.input).expect("tcPr parses");

        assert_eq!(parsed, ResolvedCellProperties { colspan: None });
    }

    #[test]
    fn parse_trpr_tblheader() {
        let mut reader = make_reader(
            r#"<w:trPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:tblHeader/></w:trPr>"#,
        );
        let _ = read_first_start(&mut reader);
        let parsed = parse_trpr(&mut reader.input).expect("trPr parses");

        assert_eq!(parsed, ResolvedRowProperties { is_header: true });
    }

    #[test]
    fn parse_trpr_empty_returns_false() {
        let mut reader = make_reader(
            r#"<w:trPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:trPr>"#,
        );
        let _ = read_first_start(&mut reader);
        let parsed = parse_trpr(&mut reader.input).expect("trPr parses");

        assert_eq!(parsed, ResolvedRowProperties { is_header: false });
    }

    #[test]
    fn parse_tc_data_cell() {
        let mut reader = make_reader(
            r#"<w:tc xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc>"#,
        );
        let start = read_first_start(&mut reader);
        reader.parse_tc(&start, false).expect("tc parses");

        assert_eq!(
            drain_queue(&mut reader),
            vec![
                start_cell(None),
                start_para(),
                Event::Text {
                    content: "cell".to_string(),
                },
                Event::EndParagraph,
                Event::EndTableCell,
            ]
        );
    }

    #[test]
    fn parse_tc_header_cell() {
        let mut reader = make_reader(
            r#"<w:tc xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>head</w:t></w:r></w:p></w:tc>"#,
        );
        let start = read_first_start(&mut reader);
        reader.parse_tc(&start, true).expect("tc parses");

        assert_eq!(
            drain_queue(&mut reader),
            vec![
                start_header(None),
                start_para(),
                Event::Text {
                    content: "head".to_string(),
                },
                Event::EndParagraph,
                Event::EndTableHeader,
            ]
        );
    }

    #[test]
    fn parse_tc_colspan() {
        let mut reader = make_reader(
            r#"<w:tc xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p/></w:tc>"#,
        );
        let start = read_first_start(&mut reader);
        reader.parse_tc(&start, false).expect("tc parses");

        assert_eq!(
            drain_queue(&mut reader),
            vec![
                start_cell(Some(2)),
                start_para(),
                Event::EndParagraph,
                Event::EndTableCell,
            ]
        );
    }

    #[test]
    fn parse_tc_second_tcpr_is_ignored() {
        let mut reader = make_reader(
            r#"<w:tc xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:tcPr><w:gridSpan w:val="1"/></w:tcPr><w:p/></w:tc>"#,
        );
        let start = read_first_start(&mut reader);
        reader.parse_tc(&start, false).expect("tc parses");

        assert_eq!(
            drain_queue(&mut reader),
            vec![
                start_cell(Some(2)),
                start_para(),
                Event::EndParagraph,
                Event::EndTableCell,
            ]
        );
    }

    #[test]
    fn parse_tr_header_row() {
        let mut reader = make_reader(
            r#"<w:tr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr>"#,
        );
        let start = read_first_start(&mut reader);
        let mut header_band_active = true;
        reader
            .parse_tr(&start, &mut header_band_active)
            .expect("tr parses");

        assert!(header_band_active);
        assert_eq!(
            drain_queue(&mut reader),
            vec![
                Event::StartTableRow { id: None },
                start_header(None),
                start_para(),
                Event::Text {
                    content: "h".to_string(),
                },
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
            ]
        );
    }

    #[test]
    fn parse_tr_second_trpr_is_ignored() {
        let mut reader = make_reader(
            r#"<w:tr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:trPr><w:tblHeader/></w:trPr><w:trPr></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr>"#,
        );
        let start = read_first_start(&mut reader);
        let mut header_band_active = true;
        reader
            .parse_tr(&start, &mut header_band_active)
            .expect("tr parses");

        assert!(header_band_active);
        assert_eq!(
            drain_queue(&mut reader),
            vec![
                Event::StartTableRow { id: None },
                start_header(None),
                start_para(),
                Event::Text {
                    content: "h".to_string(),
                },
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
            ]
        );
    }

    #[test]
    fn parse_tr_data_row_closes_band() {
        let mut reader = make_reader(
            r#"<w:tr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr>"#,
        );
        let start = read_first_start(&mut reader);
        let mut header_band_active = true;
        reader
            .parse_tr(&start, &mut header_band_active)
            .expect("tr parses");

        assert!(!header_band_active);
        assert_eq!(
            drain_queue(&mut reader),
            vec![
                Event::StartTableRow { id: None },
                start_cell(None),
                start_para(),
                Event::Text {
                    content: "d".to_string(),
                },
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
            ]
        );
    }

    #[test]
    fn parse_tr_after_band_closed() {
        let mut reader = make_reader(
            r#"<w:tr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr>"#,
        );
        let start = read_first_start(&mut reader);
        let mut header_band_active = false;
        reader
            .parse_tr(&start, &mut header_band_active)
            .expect("tr parses");

        assert!(!header_band_active);
        assert_eq!(
            drain_queue(&mut reader),
            vec![
                Event::StartTableRow { id: None },
                start_cell(None),
                start_para(),
                Event::Text {
                    content: "d".to_string(),
                },
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
            ]
        );
    }

    #[test]
    fn parse_tbl_outermost_with_header_band() {
        let doc = document_with_body(
            "<w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr></w:tbl>",
        );
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                Event::StartTable { id: None },
                Event::StartTableRow { id: None },
                start_header(None),
                start_para(),
                Event::Text {
                    content: "h".to_string(),
                },
                Event::EndParagraph,
                Event::EndTableHeader,
                Event::EndTableRow,
                Event::StartTableRow { id: None },
                start_cell(None),
                start_para(),
                Event::Text {
                    content: "d".to_string(),
                },
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn parse_tbl_nested_no_header_band() {
        let doc = document_with_body(
            "<w:tbl><w:tr><w:tc><w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr><w:tc><w:p><w:r><w:t>nested</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl>",
        );
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                Event::StartTable { id: None },
                Event::StartTableRow { id: None },
                start_cell(None),
                Event::StartTable { id: None },
                Event::StartTableRow { id: None },
                start_cell(None),
                start_para(),
                Event::Text {
                    content: "nested".to_string(),
                },
                Event::EndParagraph,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndTableCell,
                Event::EndTableRow,
                Event::EndTable,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn parse_rpr_empty_returns_default() {
        let props = parse_rpr_fragment(
            r#"<w:rPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:rPr>"#,
        );

        assert_eq!(props.kinds, Vec::<TextStyleKind>::new());
        assert_eq!(props.text_color, None);
        assert_eq!(props.mark, None);
        assert_eq!(props.font, None);
    }

    #[test]
    fn parse_rpr_accumulates_bold_italic() {
        let props = parse_rpr_fragment(
            r#"<w:rPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:b/><w:i/></w:rPr>"#,
        );

        assert_eq!(
            props.kinds,
            vec![TextStyleKind::Bold, TextStyleKind::Italic]
        );
        assert_eq!(props.text_color, None);
        assert_eq!(props.mark, None);
        assert_eq!(props.font, None);
    }

    #[test]
    fn parse_rpr_highlight_over_shading() {
        let props = parse_rpr_fragment(
            r#"<w:rPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:highlight w:val="yellow"/><w:shd w:val="clear" w:fill="FF0000"/></w:rPr>"#,
        );

        assert_eq!(props.kinds, Vec::<TextStyleKind>::new());
        assert_eq!(props.text_color, None);
        assert_eq!(
            props.mark,
            Some(Color::Rgb {
                r: 255,
                g: 255,
                b: 0,
            })
        );
        assert_eq!(props.font, None);
    }

    #[test]
    fn parse_r_text_content() {
        let doc = document_with_body("<w:p><w:r><w:t>hello</w:t></w:r></w:p>");
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::Text {
                    content: "hello".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn deleted_run_subtree_is_skipped() {
        let doc = document_with_body(
            "<w:p><w:r><w:t>A</w:t></w:r><w:del><w:r><w:t>DELETED</w:t></w:r></w:del><w:r><w:t>B</w:t></w:r></w:p>",
        );
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::Text {
                    content: "A".to_string(),
                },
                Event::Text {
                    content: "B".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn pict_subtree_without_image_data_is_skipped() {
        let doc = document_with_body(
            "<w:p><w:r><w:t>A</w:t><w:pict><v:shape><w:r><w:t>HIDDEN</w:t></w:r></v:shape></w:pict><w:t>B</w:t></w:r></w:p>",
        );
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::Text {
                    content: "A".to_string(),
                },
                Event::Text {
                    content: "B".to_string(),
                },
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn parse_r_styled_text() {
        let doc = document_with_body("<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r></w:p>");
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                Event::Text {
                    content: "bold".to_string(),
                },
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn parse_r_second_rpr_is_ignored() {
        let doc = document_with_body(
            "<w:p><w:r><w:rPr><w:b/></w:rPr><w:rPr><w:i/></w:rPr><w:t>styled</w:t></w:r></w:p>",
        );
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::StartTextStyle {
                    kind: TextStyleKind::Bold,
                    id: None,
                },
                Event::Text {
                    content: "styled".to_string(),
                },
                Event::EndTextStyle,
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn parse_r_empty_styled_run_emits_nothing() {
        let doc = document_with_body("<w:p><w:r><w:rPr><w:b/></w:rPr></w:r></w:p>");
        let mut reader = make_reader(&doc);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn drawing_embedded_internal_png_emits_zip_asset_with_alt_text() {
        let mut image_map = crate::rels::ImageMap::default();
        image_map.insert(
            "rId1".to_string(),
            image_rel("word/media/image1.png", false),
        );
        let doc = drawing_document(&picture_with_blip(
            r#"descr="alt text" name="ignored title""#,
            r#"<a:blip r:embed="rId1"/>"#,
        ));
        let mut reader = make_reader_with_images(&doc, image_map);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                image_event(
                    asset_source("zip://word/media/image1.png"),
                    Some("alt text"),
                ),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn drawing_external_link_emits_uri_without_alt() {
        let mut image_map = crate::rels::ImageMap::default();
        image_map.insert(
            "rId2".to_string(),
            image_rel("https://example.com/img.png", true),
        );
        let doc = drawing_document(&picture_with_blip("", r#"<a:blip r:link="rId2"/>"#));
        let mut reader = make_reader_with_images(&doc, image_map);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                image_event(
                    ImageSource::Uri {
                        uri: "https://example.com/img.png".to_string(),
                    },
                    None,
                ),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn drawing_missing_rid_emits_raw_asset_id() {
        let doc = drawing_document(&picture_with_blip(
            r#"descr="missing rel""#,
            r#"<a:blip r:embed="rId99"/>"#,
        ));
        let mut reader = make_reader_with_images(&doc, crate::rels::ImageMap::default());

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                image_event(asset_source("rId99"), Some("missing rel"),),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn drawing_empty_or_blip_without_ids_emits_no_image() {
        let doc = drawing_document(&picture_with_blip(r#"descr="ignored""#, "<a:blip/>"));
        let mut reader = make_reader_with_images(&doc, crate::rels::ImageMap::default());

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument
            ]
        );
    }

    #[test]
    fn drawing_with_embed_and_link_prefers_embed() {
        let mut image_map = crate::rels::ImageMap::default();
        image_map.insert(
            "rIdEmbed".to_string(),
            image_rel("word/media/embed.png", false),
        );
        image_map.insert(
            "rIdLink".to_string(),
            image_rel("https://example.com/link.png", true),
        );
        let doc = drawing_document(&picture_with_blip(
            r#"descr="embed wins""#,
            r#"<a:blip r:embed="rIdEmbed" r:link="rIdLink"/>"#,
        ));
        let mut reader = make_reader_with_images(&doc, image_map);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                image_event(
                    asset_source("zip://word/media/embed.png"),
                    Some("embed wins"),
                ),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn drawing_embed_with_external_target_mode_emits_uri() {
        let mut image_map = crate::rels::ImageMap::default();
        image_map.insert(
            "rIdExternalEmbed".to_string(),
            image_rel("https://cdn.example.com/embed.png", true),
        );
        let doc = drawing_document(&picture_with_blip(
            r#"descr="external embed""#,
            r#"<a:blip r:embed="rIdExternalEmbed"/>"#,
        ));
        let mut reader = make_reader_with_images(&doc, image_map);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                image_event(
                    ImageSource::Uri {
                        uri: "https://cdn.example.com/embed.png".to_string(),
                    },
                    Some("external embed"),
                ),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn drawing_ignores_smart_art_blip_outside_picture_blip_fill() {
        let mut image_map = crate::rels::ImageMap::default();
        image_map.insert(
            "rIdSmart".to_string(),
            image_rel("word/media/smart.png", false),
        );
        let doc = drawing_document(
            r#"<wp:inline><wp:docPr descr="ignored"/><a:graphic><a:graphicData><a:blip r:embed="rIdSmart"/></a:graphicData></a:graphic></wp:inline>"#,
        );
        let mut reader = make_reader_with_images(&doc, image_map);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                Event::EndParagraph,
                Event::EndDocument
            ]
        );
    }

    #[test]
    fn drawing_two_pictures_emit_two_images_in_source_order() {
        let mut image_map = crate::rels::ImageMap::default();
        image_map.insert("rId1".to_string(), image_rel("word/media/one.png", false));
        image_map.insert("rId2".to_string(), image_rel("word/media/two.png", false));
        let doc = drawing_document(&format!(
            "{}{}",
            picture_with_blip(r#"descr="one""#, r#"<a:blip r:embed="rId1"/>"#),
            picture_with_blip(r#"descr="two""#, r#"<a:blip r:embed="rId2"/>"#),
        ));
        let mut reader = make_reader_with_images(&doc, image_map);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                image_event(asset_source("zip://word/media/one.png"), Some("one"),),
                image_event(asset_source("zip://word/media/two.png"), Some("two"),),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn drawing_non_empty_blip_element_emits_like_self_closing_blip() {
        let mut image_map = crate::rels::ImageMap::default();
        image_map.insert(
            "rId1".to_string(),
            image_rel("word/media/image1.png", false),
        );
        let doc = drawing_document(&picture_with_blip(
            r#"descr="start tag""#,
            r#"<a:blip r:embed="rId1"><a:alphaModFix/></a:blip>"#,
        ));
        let mut reader = make_reader_with_images(&doc, image_map);

        assert_eq!(
            collect_events(&mut reader),
            vec![
                start_doc(),
                start_para(),
                image_event(
                    asset_source("zip://word/media/image1.png"),
                    Some("start tag"),
                ),
                Event::EndParagraph,
                Event::EndDocument,
            ]
        );
    }

    #[test]
    fn pstyle_heading1_emits_start_heading() {
        let styles = r#"<w:style w:type="paragraph" w:styleId="Heading1">
            <w:name w:val="heading 1"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Hello</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartHeading { level: 1, id: None },
                docspec_core::Event::Text {
                    content: "Hello".to_string(),
                },
                docspec_core::Event::EndHeading,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn pstyle_title_folds_to_heading1() {
        let styles = r#"<w:style w:type="paragraph" w:styleId="Title">
            <w:name w:val="Title"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Title"/></w:pPr>
      <w:r><w:t>My Title</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartHeading { level: 1, id: None },
                docspec_core::Event::Text {
                    content: "My Title".to_string(),
                },
                docspec_core::Event::EndHeading,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn pstyle_block_quote_emits_start_block_quote() {
        let styles = r#"<w:style w:type="paragraph" w:styleId="BlockQuote">
            <w:name w:val="Block Quote"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="BlockQuote"/></w:pPr>
      <w:r><w:t>quoted</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartBlockQuote { id: None },
                docspec_core::Event::Text {
                    content: "quoted".to_string(),
                },
                docspec_core::Event::EndBlockQuote,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn pstyle_source_code_emits_start_preformatted() {
        let styles = r#"<w:style w:type="paragraph" w:styleId="SourceCode">
            <w:name w:val="Source Code"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="SourceCode"/></w:pPr>
      <w:r><w:t>fn main() {}</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartPreformatted {
                    id: None,
                    syntax: None,
                },
                docspec_core::Event::Text {
                    content: "fn main() {}".to_string(),
                },
                docspec_core::Event::EndPreformatted,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn pstyle_heading_99_emits_level_99() {
        let styles = r#"<w:style w:type="paragraph" w:styleId="Heading99">
            <w:name w:val="heading 99"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading99"/></w:pPr>
      <w:r><w:t>deep</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartHeading {
                    level: 99,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "deep".to_string(),
                },
                docspec_core::Event::EndHeading,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn pstyle_unknown_id_falls_through_to_paragraph() {
        let styles = r#"<w:style w:type="paragraph" w:styleId="Normal">
            <w:name w:val="Normal"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="DoesNotExist"/></w:pPr>
      <w:r><w:t>plain</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "plain".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn pstyle_no_pstyle_emits_paragraph() {
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>bare</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, "");
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "bare".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn pstyle_out_of_order_ppr_ignored() {
        let styles = r#"<w:style w:type="paragraph" w:styleId="Heading1">
            <w:name w:val="heading 1"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>text</w:t></w:r><w:pPr><w:pStyle w:val="Heading1"/></w:pPr></w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "text".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn pstyle_chain_walk_resolves_based_on() {
        let styles = r#"<w:style w:type="paragraph" w:styleId="Heading2">
            <w:name w:val="heading 2"/>
        </w:style>
        <w:style w:type="paragraph" w:styleId="MyHeading">
            <w:basedOn w:val="Heading2"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="MyHeading"/></w:pPr>
      <w:r><w:t>section</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartHeading { level: 2, id: None },
                docspec_core::Event::Text {
                    content: "section".to_string(),
                },
                docspec_core::Event::EndHeading,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn rstyle_code_classification_emits_inline_code_wrapper() {
        let styles = r#"<w:style w:type="character" w:styleId="CodeChar">
            <w:name w:val="Verbatim Char"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:rPr><w:rStyle w:val="CodeChar"/></w:rPr><w:t>x</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::StartTextStyle {
                    kind: docspec_core::TextStyleKind::Code,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "x".to_string(),
                },
                docspec_core::Event::EndTextStyle,
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn rstyle_unknown_classification_emits_no_wrapper() {
        let styles = r#"<w:style w:type="character" w:styleId="CodeChar">
            <w:name w:val="FooBar"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:rPr><w:rStyle w:val="CodeChar"/></w:rPr><w:t>x</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "x".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn rstyle_non_code_classification_emits_no_wrapper() {
        let styles = r#"<w:style w:type="character" w:styleId="CodeChar">
            <w:name w:val="heading 1"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:rPr><w:rStyle w:val="CodeChar"/></w:rPr><w:t>x</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "x".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn rstyle_inside_ppr_rpr_is_ignored() {
        let styles = r#"<w:style w:type="character" w:styleId="CodeChar">
            <w:name w:val="Verbatim Char"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:rPr><w:rStyle w:val="CodeChar"/></w:rPr></w:pPr>
      <w:r><w:t>x</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "x".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn rstyle_duplicate_rstyle_emits_single_wrapper() {
        let styles = r#"<w:style w:type="character" w:styleId="CodeChar">
            <w:name w:val="Verbatim Char"/>
        </w:style>"#;
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:rPr><w:rStyle w:val="CodeChar"/><w:rStyle w:val="CodeChar"/></w:rPr><w:t>x</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader_with_styles(doc, styles);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::StartTextStyle {
                    kind: docspec_core::TextStyleKind::Code,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "x".to_string(),
                },
                docspec_core::Event::EndTextStyle,
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn single_ordered_list_item_emits_correct_events() {
        let doc = document_with_body(&list_paragraph(1, 0, "item"));
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "item".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn nested_list_items_emit_correct_events() {
        let body = format!(
            "{}{}",
            list_paragraph(1, 0, "parent"),
            list_paragraph(1, 1, "child")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "parent".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 1,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "child".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn level_decrease_pops_stack_correctly() {
        let body = format!(
            "{}{}{}",
            list_paragraph(1, 0, "one"),
            list_paragraph(1, 1, "child"),
            list_paragraph(1, 0, "two")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "one".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 1,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "child".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "two".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn non_list_paragraph_between_list_items_breaks_list() {
        let body = format!(
            "{}{}{}",
            list_paragraph(1, 0, "one"),
            plain_paragraph("plain"),
            list_paragraph(1, 0, "two")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "one".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "plain".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(2),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "two".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn document_end_flush_closes_open_list() {
        let doc = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{}"#,
            r#"<w:p><w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr><w:r><w:t>item</w:t></w:r>"#
        );
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "item".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn num_id_zero_sentinel_emits_plain_paragraph() {
        let doc = document_with_body(&list_paragraph(0, 0, "plain"));
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "plain".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn oversized_list_level_is_clamped_to_eight() {
        let doc = document_with_body(&list_paragraph(1, 99, "deep"));
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 1,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 2,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 3,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 4,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 5,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 6,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 7,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 8,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "deep".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn list_item_inside_table_cell_closes_before_cell_end() {
        let doc = document_with_body(&format!(
            "<w:tbl><w:tr><w:tc>{}</w:tc></w:tr></w:tbl>",
            list_paragraph(1, 0, "cell")
        ));
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartTable { id: None },
                docspec_core::Event::StartTableRow { id: None },
                docspec_core::Event::StartTableCell {
                    colspan: None,
                    rowspan: None,
                    id: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "cell".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndTableCell,
                docspec_core::Event::EndTableRow,
                docspec_core::Event::EndTable,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn table_after_list_item_breaks_list_before_table_start() {
        let doc = document_with_body(&format!(
            "{}<w:tbl><w:tr><w:tc>{}</w:tc></w:tr></w:tbl>",
            list_paragraph(1, 0, "item"),
            plain_paragraph("cell")
        ));
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "item".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartTable { id: None },
                docspec_core::Event::StartTableRow { id: None },
                docspec_core::Event::StartTableCell {
                    colspan: None,
                    rowspan: None,
                    id: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "cell".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndTableCell,
                docspec_core::Event::EndTableRow,
                docspec_core::Event::EndTable,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn numpr_with_num_id_and_ilvl_is_consumed_by_paragraph_start() {
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="3"/><w:ilvl w:val="2"/></w:numPr></w:pPr>
      <w:r><w:t>item</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader(doc);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "item".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn numpr_with_num_id_only_defaults_ilvl_to_zero_before_consuming() {
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>item</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader(doc);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "item".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn numpr_without_num_id_emits_plain_paragraph() {
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>plain</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut reader = make_reader(doc);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "plain".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    fn make_reader_with_styles_and_numbering(
        document_xml: &str,
        styles_body: &str,
        numbering: crate::numbering::MinimalNumbering,
    ) -> DocumentReader {
        let xml_str = styles_xml(styles_body);
        let style_list = crate::styles::StyleList::parse(Cursor::new(xml_str.into_bytes()))
            .expect("valid styles XML");
        let stream: Box<dyn Read + Send> = Box::new(Cursor::new(document_xml.as_bytes().to_vec()));
        let xml = quick_xml::Reader::from_reader(std::io::BufReader::new(stream));
        let data = DocxData {
            style_list,
            hyperlink_map: HyperlinkMap::default(),
            numbering,
            image_map: crate::rels::ImageMap::default(),
        };
        DocumentReader::from_xml_reader(xml, data)
    }

    #[test]
    fn multiple_plain_paragraphs_after_list_item_emit_as_top_level_siblings() {
        let body = format!(
            "{}{}{}{}",
            list_paragraph(1, 0, "one"),
            plain_paragraph("first continuation"),
            plain_paragraph("second continuation"),
            list_paragraph(1, 0, "two")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "one".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "first continuation".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "second continuation".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(2),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "two".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn trailing_plain_paragraphs_after_list_at_document_end_emit_as_top_level_siblings() {
        let body = format!(
            "{}{}{}",
            list_paragraph(1, 0, "one"),
            plain_paragraph("trailing first"),
            plain_paragraph("trailing second")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "one".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "trailing first".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "trailing second".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn plain_paragraph_then_deeper_level_starts_new_list_with_phantom_level_zero_wrapper() {
        let body = format!(
            "{}{}{}",
            list_paragraph(1, 0, "outer"),
            plain_paragraph("continuation"),
            list_paragraph(1, 1, "child")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "outer".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "continuation".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 1,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "child".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn plain_paragraph_inside_table_cell_breaks_list_immediately() {
        let body = format!(
            "<w:tbl><w:tr><w:tc>{}{}</w:tc></w:tr></w:tbl>",
            list_paragraph(1, 0, "cell item"),
            plain_paragraph("continuation")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartTable { id: None },
                docspec_core::Event::StartTableRow { id: None },
                docspec_core::Event::StartTableCell {
                    colspan: None,
                    rowspan: None,
                    id: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "cell item".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "continuation".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndTableCell,
                docspec_core::Event::EndTableRow,
                docspec_core::Event::EndTable,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn heading_between_list_items_still_closes_list() {
        let styles_body = r#"<w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
</w:style>"#;
        let body = format!(
            "{}{}{}",
            list_paragraph(1, 0, "one"),
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>heading</w:t></w:r></w:p>"#,
            list_paragraph(1, 0, "two")
        );
        let doc = document_with_body(&body);
        let mut reader =
            make_reader_with_styles_and_numbering(&doc, styles_body, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "one".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartHeading { level: 1, id: None },
                docspec_core::Event::Text {
                    content: "heading".to_string(),
                },
                docspec_core::Event::EndHeading,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(2),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "two".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn continuation_paragraph_before_any_list_emits_plain() {
        let body = format!(
            "{}{}",
            plain_paragraph("standalone"),
            list_paragraph(1, 0, "item")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "standalone".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "item".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn calibre_demo_continued_lists_pattern_produces_continuous_numbering() {
        let body = format!(
            "{}{}{}{}{}",
            list_paragraph(7, 0, "One"),
            list_paragraph(7, 0, "Two"),
            plain_paragraph(
                "An interruption in our regularly scheduled listing, for this essential and very relevant public service announcement."
            ),
            list_paragraph(7, 0, "We now resume our normal programming"),
            list_paragraph(7, 0, "Four")
        );
        let doc = document_with_body(&body);
        let numbering_xml_str = numbering_xml(
            r#"<w:abstractNum w:abstractNumId="7">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
</w:abstractNum>
<w:num w:numId="7"><w:abstractNumId w:val="7"/></w:num>"#,
        );
        let numbering =
            crate::numbering::parse_numbering(Cursor::new(numbering_xml_str.into_bytes()))
                .expect("valid numbering XML");
        let mut reader = make_reader_with_numbering(&doc, numbering);
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("7".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "One".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("7".to_string()),
                    level: 0,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "Two".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "An interruption in our regularly scheduled listing, for this essential and very relevant public service announcement.".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("7".to_string()),
                    level: 0,
                    start: Some(3),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "We now resume our normal programming".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("7".to_string()),
                    level: 0,
                    start: None,
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "Four".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn different_num_id_at_same_level_emits_two_distinct_lists() {
        let body = format!(
            "{}{}",
            list_paragraph(1, 0, "first"),
            list_paragraph(2, 0, "second")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, two_decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "first".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("2".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "second".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn plain_paragraph_between_two_same_num_id_lists_breaks_list_preserves_counter() {
        let body = format!(
            "{}{}{}",
            list_paragraph(1, 0, "one"),
            plain_paragraph("interrupt"),
            list_paragraph(1, 0, "two")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "one".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "interrupt".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(2),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "two".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn trailing_plain_paragraphs_after_list_emit_as_top_level_siblings() {
        let body = format!(
            "{}{}{}",
            list_paragraph(1, 0, "item"),
            plain_paragraph("tail1"),
            plain_paragraph("tail2")
        );
        let doc = document_with_body(&body);
        let mut reader = make_reader_with_numbering(&doc, decimal_numbering());
        let events = collect_events(&mut reader);
        assert_eq!(
            events,
            vec![
                docspec_core::Event::StartDocument {
                    id: None,
                    language: None,
                    metadata: None,
                },
                docspec_core::Event::StartOrderedListItem {
                    id: Some("1".to_string()),
                    level: 0,
                    start: Some(1),
                    style_type: ListStyleType::Decimal,
                },
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "item".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "tail1".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "tail2".to_string(),
                },
                docspec_core::Event::EndParagraph,
                docspec_core::Event::EndDocument,
            ]
        );
    }
}
