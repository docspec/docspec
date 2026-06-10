//! DOCX main document part (`document.xml`) streaming event parser.

use alloc::collections::VecDeque;
use core::fmt;
use std::io::{BufReader, Read};

use docspec_core::{Color, Error, Event, Result, TextAlignment, TextStyleKind};
use quick_xml::events::{BytesCData, BytesRef, BytesStart, BytesText};

use crate::properties;
use crate::styles::StyleList;

/// Document processing phase.
#[derive(Clone, Copy, PartialEq, Eq)]
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
/// Set in `ensure_paragraph_started()` based on `pending_paragraph_classification`.
/// Consumed in `end_paragraph()` to emit the matching End event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParagraphBlockKind {
    /// Plain paragraph (default).
    Paragraph,
    /// Heading at the given level.
    Heading { level: u8 },
    /// Block quotation.
    BlockQuote,
    /// Preformatted / code block.
    Preformatted,
}

/// Bundle of package-level data the document reader consults during streaming.
///
/// Forward-compatible: additional package parts (theme, numbering, settings)
/// will be added as fields here without breaking the constructor signature.
pub struct DocxData {
    /// Styles loaded from the styles part, used to classify paragraph and run styles.
    pub style_list: StyleList,
}

/// Streaming parser for the DOCX main document XML part.
#[expect(
    clippy::struct_excessive_bools,
    reason = "DocumentReader tracks six independent boolean parser states; grouping them would obscure the streaming state machine"
)]
pub struct DocumentReader {
    /// Reusable buffer for quick-xml event reading.
    buf: Vec<u8>,
    /// Depth counter for ignored subtrees (tracked changes, hyperlinks,
    /// drawings, table/row/cell property containers, etc.).
    /// Incremented on Start of an ignored container, decremented on End.
    in_ignored_subtree: u32,
    /// Whether the reader is currently inside a `<w:p>` element.
    in_paragraph: bool,
    /// Whether the reader is currently inside a `<w:t>` element.
    in_text: bool,
    /// Whether currently inside a `<w:pPr>` element that is still legal (first child of paragraph).
    in_ppr: bool,
    /// Paragraph alignment captured from `<w:jc>` while inside `<w:pPr>`.
    pending_paragraph_alignment: Option<TextAlignment>,
    /// Style classification pending for the current paragraph, set by `<w:pStyle>` parsing.
    /// Consumed and cleared by `ensure_paragraph_started()`.
    pending_paragraph_classification: Option<crate::styles::StyleClassification>,
    /// The block kind opened for the current paragraph.
    /// Set in `ensure_paragraph_started()`, consumed in `end_paragraph()`.
    current_paragraph_block: ParagraphBlockKind,
    /// True once `StartParagraph` has been queued for the current paragraph.
    paragraph_started_emitted: bool,
    /// Whether currently inside a `<w:rPr>` element that is still legal (first child of run).
    in_rpr: bool,
    /// Run style kinds accumulated while inside `<w:rPr>`.
    pending_run_kinds: Vec<TextStyleKind>,
    /// Text color accumulated while inside `<w:rPr>`.
    pending_run_text_color: Option<Color>,
    /// Highlight color accumulated while inside `<w:rPr>`.
    pending_run_mark: Option<Color>,
    /// Shading fill color accumulated while inside `<w:rPr>`.
    pending_run_shade: Option<Color>,
    /// Text collected for the current `<w:t>` element.
    pending_text: String,
    /// Run style kinds frozen at `</w:rPr>`, applied to subsequent emissions in the same run.
    frozen_run_kinds: Vec<TextStyleKind>,
    /// Text color frozen at `</w:rPr>`, applied to subsequent emissions in the same run.
    frozen_run_text_color: Option<Color>,
    /// Highlight/background color frozen at `</w:rPr>`, applied to subsequent emissions in the same run.
    frozen_run_mark: Option<Color>,
    /// Style kinds currently opened for the active run.
    open_styles: Vec<TextStyleKind>,
    /// Document processing phase.
    phase: Phase,
    /// Queue of `DocSpec` events to emit.
    queue: VecDeque<Event>,
    /// True once the first content event of the current run has been queued.
    run_content_emitted: bool,
    /// Package-level data (styles, future: theme, numbering).
    data: DocxData,
    /// The quick-xml reader streaming from the document entry.
    xml: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>,
}

impl fmt::Debug for DocumentReader {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocumentReader")
            .field("buf", &self.buf)
            .field("in_ignored_subtree", &self.in_ignored_subtree)
            .field("in_paragraph", &self.in_paragraph)
            .field("in_text", &self.in_text)
            .field("in_ppr", &self.in_ppr)
            .field(
                "pending_paragraph_alignment",
                &self.pending_paragraph_alignment,
            )
            .field(
                "pending_paragraph_classification",
                &self.pending_paragraph_classification,
            )
            .field("current_paragraph_block", &self.current_paragraph_block)
            .field("paragraph_started_emitted", &self.paragraph_started_emitted)
            .field("in_rpr", &self.in_rpr)
            .field("pending_run_kinds", &self.pending_run_kinds)
            .field("pending_run_text_color", &self.pending_run_text_color)
            .field("pending_run_mark", &self.pending_run_mark)
            .field("pending_run_shade", &self.pending_run_shade)
            .field("pending_text", &self.pending_text)
            .field("frozen_run_kinds", &self.frozen_run_kinds)
            .field("frozen_run_text_color", &self.frozen_run_text_color)
            .field("frozen_run_mark", &self.frozen_run_mark)
            .field("open_styles", &self.open_styles)
            .field("phase", &"<phase>")
            .field("queue", &self.queue)
            .field("run_content_emitted", &self.run_content_emitted)
            .field("data", &"<DocxData>")
            .field("xml", &"<quick_xml::Reader>")
            .finish()
    }
}

impl DocumentReader {
    pub fn from_xml_reader(
        xml: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>,
        data: DocxData,
    ) -> Self {
        Self {
            buf: Vec::with_capacity(4096),
            in_ignored_subtree: 0,
            in_paragraph: false,
            in_text: false,
            in_ppr: false,
            pending_paragraph_alignment: None,
            pending_paragraph_classification: None,
            current_paragraph_block: ParagraphBlockKind::Paragraph,
            paragraph_started_emitted: false,
            in_rpr: false,
            pending_run_kinds: Vec::new(),
            pending_run_text_color: None,
            pending_run_mark: None,
            pending_run_shade: None,
            pending_text: String::new(),
            frozen_run_kinds: Vec::new(),
            frozen_run_text_color: None,
            frozen_run_mark: None,
            open_styles: Vec::new(),
            phase: Phase::NotStarted,
            queue: VecDeque::new(),
            run_content_emitted: false,
            data,
            xml,
        }
    }
}

impl DocumentReader {
    fn can_collect_text(&self) -> bool {
        self.in_ignored_subtree == 0 && self.in_paragraph && self.in_text
    }

    fn emit_line_break(&mut self) {
        self.ensure_paragraph_started();
        self.flush_pending_text();
        self.emit_deferred_starts();
        self.run_content_emitted = true;
        self.queue.push_back(Event::LineBreak);
    }

    fn emit_tab(&mut self) {
        self.ensure_paragraph_started();
        self.flush_pending_text();
        self.emit_deferred_starts();
        self.run_content_emitted = true;
        self.queue.push_back(Event::Text {
            content: "\t".to_string(),
        });
    }

    fn end_paragraph(&mut self) {
        self.ensure_paragraph_started();
        while self.open_styles.pop().is_some() {
            self.queue.push_back(Event::EndTextStyle);
        }
        self.frozen_run_kinds.clear();
        self.pending_run_kinds.clear();
        self.frozen_run_text_color = None;
        self.frozen_run_mark = None;
        self.pending_run_text_color = None;
        self.pending_run_mark = None;
        self.pending_run_shade = None;
        let end_event = match self.current_paragraph_block {
            ParagraphBlockKind::Paragraph => Event::EndParagraph,
            ParagraphBlockKind::Heading { .. } => Event::EndHeading,
            ParagraphBlockKind::BlockQuote => Event::EndBlockQuote,
            ParagraphBlockKind::Preformatted => Event::EndPreformatted,
        };
        self.queue.push_back(end_event);
        self.in_paragraph = false;
        self.in_text = false;
        self.pending_text.clear();
        self.in_ppr = false;
        self.pending_paragraph_alignment = None;
        self.pending_paragraph_classification = None;
        self.current_paragraph_block = ParagraphBlockKind::Paragraph;
        self.paragraph_started_emitted = false;
    }

    fn flush_pending_text(&mut self) {
        if !self.pending_text.is_empty() {
            self.emit_deferred_starts();
            self.queue.push_back(Event::Text {
                content: core::mem::take(&mut self.pending_text),
            });
        }
    }

    fn emit_deferred_starts(&mut self) {
        for kind in &self.frozen_run_kinds {
            if !self.open_styles.contains(kind) {
                self.queue.push_back(Event::StartTextStyle {
                    kind: kind.clone(),
                    id: None,
                });
                self.open_styles.push(kind.clone());
            }
        }
        if let Some(color) = self.frozen_run_text_color.clone() {
            let kind = TextStyleKind::TextColor(color);
            if !self.open_styles.contains(&kind) {
                self.queue.push_back(Event::StartTextStyle {
                    kind: kind.clone(),
                    id: None,
                });
                self.open_styles.push(kind);
            }
        }
        if let Some(color) = self.frozen_run_mark.clone() {
            let kind = TextStyleKind::Mark(color);
            if !self.open_styles.contains(&kind) {
                self.queue.push_back(Event::StartTextStyle {
                    kind: kind.clone(),
                    id: None,
                });
                self.open_styles.push(kind);
            }
        }
    }

    fn set_pending_run_kind(&mut self, kind: TextStyleKind, enabled: bool) {
        self.pending_run_kinds.retain(|current| current != &kind);
        if enabled {
            self.pending_run_kinds.push(kind);
        }
    }

    fn set_pending_vertical_alignment(&mut self, align: properties::VertAlign) {
        self.pending_run_kinds.retain(|kind| {
            kind != &TextStyleKind::Subscript && kind != &TextStyleKind::Superscript
        });
        match align {
            properties::VertAlign::Subscript => {
                self.pending_run_kinds.push(TextStyleKind::Subscript);
            }
            properties::VertAlign::Superscript => {
                self.pending_run_kinds.push(TextStyleKind::Superscript);
            }
            properties::VertAlign::None => {}
        }
    }

    fn handle_rpr_rstyle(&mut self, tag: &BytesStart<'_>) {
        if let Some(crate::styles::StyleClassification::Code) = read_val_attribute(tag)
            .filter(|s| !s.is_empty())
            .and_then(|s| self.data.style_list.classify(&s))
        {
            if !self.pending_run_kinds.contains(&TextStyleKind::Code) {
                self.pending_run_kinds.push(TextStyleKind::Code);
            }
        }
    }

    fn handle_cdata(&mut self, cdata: BytesCData<'_>) -> Result<()> {
        if self.can_collect_text() {
            let bytes = cdata.into_inner();
            let content = core::str::from_utf8(&bytes)
                .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
            self.pending_text.push_str(content);
        }
        Ok(())
    }

    fn handle_empty(&mut self, tag: &BytesStart<'_>) {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        match local {
            value if self.in_ignored_subtree > 0 || is_ignored_container(value) => {}
            b"pPr" if self.in_paragraph && !self.paragraph_started_emitted => {
                self.ensure_paragraph_started();
            }
            b"jc" if self.in_ppr => {
                let val = read_val_attribute(tag);
                self.pending_paragraph_alignment =
                    val.as_deref().and_then(properties::parse_alignment);
            }
            b"pStyle" if self.in_ppr && !self.paragraph_started_emitted => {
                self.pending_paragraph_classification = read_val_attribute(tag)
                    .filter(|s| !s.is_empty())
                    .and_then(|s| self.data.style_list.classify(&s));
            }
            b"rPr" if self.in_ppr => {}
            b"rPr" if self.in_paragraph && !self.in_ppr && !self.in_rpr => {}
            b"b" if self.in_rpr => {
                self.set_pending_run_kind(TextStyleKind::Bold, parse_on_off_attribute(tag));
            }
            b"i" if self.in_rpr => {
                self.set_pending_run_kind(TextStyleKind::Italic, parse_on_off_attribute(tag));
            }
            b"strike" | b"dstrike" if self.in_rpr => {
                self.set_pending_run_kind(
                    TextStyleKind::Strikethrough,
                    parse_on_off_attribute(tag),
                );
            }
            b"u" if self.in_rpr => {
                let val = read_val_attribute(tag);
                self.set_pending_run_kind(
                    TextStyleKind::Underline,
                    properties::parse_underline_on(val.as_deref()),
                );
            }
            b"vertAlign" if self.in_rpr => {
                let val = read_val_attribute(tag);
                self.set_pending_vertical_alignment(properties::parse_vert_align(val.as_deref()));
            }
            b"color" if self.in_rpr => {
                let val = read_val_attribute(tag);
                self.pending_run_text_color = properties::parse_color_val(val.as_deref());
            }
            b"highlight" if self.in_rpr => {
                let val = read_val_attribute(tag);
                self.pending_run_mark = properties::parse_highlight_val(val.as_deref());
            }
            b"shd" if self.in_rpr => {
                let fill = read_attribute(tag, b"w:fill");
                self.pending_run_shade = properties::parse_shd_fill(fill.as_deref());
            }
            b"rStyle" if self.in_rpr => self.handle_rpr_rstyle(tag),
            b"p" if !self.in_paragraph => {
                self.start_paragraph();
                self.end_paragraph();
            }
            b"br" if self.in_paragraph => self.emit_line_break(),
            b"tab" if self.in_paragraph => self.emit_tab(),
            _ => {}
        }
    }

    fn handle_end(&mut self, local: &[u8]) {
        if self.in_ignored_subtree > 0 {
            self.in_ignored_subtree = self.in_ignored_subtree.saturating_sub(1);
            return;
        }

        match local {
            b"p" if self.in_paragraph => self.end_paragraph(),
            b"pPr" if self.in_ppr => {
                self.ensure_paragraph_started();
                self.in_ppr = false;
            }
            b"rPr" if self.in_rpr => {
                self.frozen_run_kinds = core::mem::take(&mut self.pending_run_kinds);
                self.frozen_run_text_color = self.pending_run_text_color.take();
                self.frozen_run_mark = self
                    .pending_run_mark
                    .take()
                    .or_else(|| self.pending_run_shade.take());
                self.pending_run_shade = None;
                self.in_rpr = false;
            }
            b"r" => {
                while self.open_styles.pop().is_some() {
                    self.queue.push_back(Event::EndTextStyle);
                }
                self.frozen_run_kinds.clear();
                self.pending_run_kinds.clear();
                self.frozen_run_text_color = None;
                self.frozen_run_mark = None;
                self.pending_run_text_color = None;
                self.pending_run_mark = None;
                self.pending_run_shade = None;
                self.run_content_emitted = false;
                self.in_rpr = false;
            }
            b"t" if self.in_text => {
                self.flush_pending_text();
                self.in_text = false;
            }
            b"tbl" => self.queue.push_back(Event::EndTable),
            b"tr" => self.queue.push_back(Event::EndTableRow),
            b"tc" => self.queue.push_back(Event::EndTableCell),
            _ => {}
        }
    }

    fn handle_eof(&mut self) {
        if self.in_text {
            self.flush_pending_text();
        }
        if self.in_paragraph {
            self.end_paragraph();
        }
        self.queue.push_back(Event::EndDocument);
        self.phase = Phase::Finished;
    }

    fn handle_general_ref(&mut self, reference: &BytesRef<'_>) -> Result<()> {
        if self.can_collect_text() {
            let decoded = reference
                .decode()
                .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
            let escaped = format!("&{decoded};");
            let unescaped = quick_xml::escape::unescape(&escaped)
                .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
            self.pending_text.push_str(&unescaped);
        }
        Ok(())
    }

    fn handle_start(&mut self, tag: &BytesStart<'_>) {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        if self.in_ignored_subtree > 0 {
            self.in_ignored_subtree = self.in_ignored_subtree.saturating_add(1);
            return;
        }

        match local {
            value if is_ignored_container(value) => self.in_ignored_subtree = 1,
            b"pPr" if self.in_paragraph => {
                if self.paragraph_started_emitted {
                    // Out-of-order pPr: StartParagraph already emitted; silently consume
                    self.in_ignored_subtree = 1;
                } else {
                    self.in_ppr = true;
                    self.pending_paragraph_alignment = None;
                }
            }
            b"jc" if self.in_ppr => {
                let val = read_val_attribute(tag);
                self.pending_paragraph_alignment =
                    val.as_deref().and_then(properties::parse_alignment);
            }
            b"pStyle" if self.in_ppr && !self.paragraph_started_emitted => {
                self.pending_paragraph_classification = read_val_attribute(tag)
                    .filter(|s| !s.is_empty())
                    .and_then(|s| self.data.style_list.classify(&s));
            }
            b"rPr" if self.in_ppr => {
                self.in_ignored_subtree = 1;
            }
            b"rPr" if self.in_paragraph && !self.in_ppr && !self.in_rpr => {
                if self.run_content_emitted {
                    // Out-of-order rPr: content already emitted in this run; silently consume
                    self.in_ignored_subtree = 1;
                } else {
                    self.in_rpr = true;
                    self.pending_run_kinds.clear();
                    self.pending_run_text_color = None;
                    self.pending_run_mark = None;
                    self.pending_run_shade = None;
                }
            }
            b"b" if self.in_rpr => {
                self.set_pending_run_kind(TextStyleKind::Bold, parse_on_off_attribute(tag));
            }
            b"i" if self.in_rpr => {
                self.set_pending_run_kind(TextStyleKind::Italic, parse_on_off_attribute(tag));
            }
            b"strike" | b"dstrike" if self.in_rpr => {
                self.set_pending_run_kind(
                    TextStyleKind::Strikethrough,
                    parse_on_off_attribute(tag),
                );
            }
            b"u" if self.in_rpr => {
                let val = read_val_attribute(tag);
                self.set_pending_run_kind(
                    TextStyleKind::Underline,
                    properties::parse_underline_on(val.as_deref()),
                );
            }
            b"vertAlign" if self.in_rpr => {
                let val = read_val_attribute(tag);
                self.set_pending_vertical_alignment(properties::parse_vert_align(val.as_deref()));
            }
            b"color" if self.in_rpr => {
                let val = read_val_attribute(tag);
                self.pending_run_text_color = properties::parse_color_val(val.as_deref());
            }
            b"highlight" if self.in_rpr => {
                let val = read_val_attribute(tag);
                self.pending_run_mark = properties::parse_highlight_val(val.as_deref());
            }
            b"shd" if self.in_rpr => {
                let fill = read_attribute(tag, b"w:fill");
                self.pending_run_shade = properties::parse_shd_fill(fill.as_deref());
            }
            b"rStyle" if self.in_rpr => self.handle_rpr_rstyle(tag),
            b"p" if !self.in_paragraph => self.start_paragraph(),
            b"r" if self.in_paragraph => {
                self.ensure_paragraph_started();
            }
            b"t" if self.in_paragraph => {
                self.ensure_paragraph_started();
                self.in_text = true;
                self.pending_text.clear();
                self.run_content_emitted = true;
            }
            b"br" if self.in_paragraph => self.emit_line_break(),
            b"tab" if self.in_paragraph => self.emit_tab(),
            b"tbl" => self.queue.push_back(Event::StartTable { id: None }),
            b"tr" => self.queue.push_back(Event::StartTableRow { id: None }),
            b"tc" => self.queue.push_back(Event::StartTableCell {
                colspan: None,
                id: None,
                rowspan: None,
            }),
            _ => {}
        }
    }

    fn handle_text(&mut self, text: &BytesText<'_>) -> Result<()> {
        if self.can_collect_text() {
            let decoded = text
                .decode()
                .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
            let unescaped = quick_xml::escape::unescape(&decoded)
                .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
            self.pending_text.push_str(&unescaped);
        }
        Ok(())
    }

    fn read_until_event(&mut self) -> Result<()> {
        let event = self
            .xml
            .read_event_into(&mut self.buf)
            .map_err(|err| match err {
                quick_xml::Error::Io(source) => Error::Io {
                    source: std::io::Error::new(source.kind(), source.to_string()),
                },
                other => Error::Parse {
                    message: format!("malformed document.xml: {other}"),
                    position: None,
                },
            })?
            .into_owned();

        match event {
            quick_xml::events::Event::Start(tag) => self.handle_start(&tag),
            quick_xml::events::Event::End(tag) => self.handle_end(tag.local_name().as_ref()),
            quick_xml::events::Event::Empty(tag) => self.handle_empty(&tag),
            quick_xml::events::Event::Text(text) => {
                self.handle_text(&text)?;
            }
            quick_xml::events::Event::GeneralRef(reference) => {
                self.handle_general_ref(&reference)?;
            }
            quick_xml::events::Event::CData(cdata) => self.handle_cdata(cdata)?,
            quick_xml::events::Event::Eof => self.handle_eof(),
            quick_xml::events::Event::Comment(_)
            | quick_xml::events::Event::Decl(_)
            | quick_xml::events::Event::PI(_)
            | quick_xml::events::Event::DocType(_) => {}
        }

        self.buf.clear();
        Ok(())
    }

    fn start_paragraph(&mut self) {
        self.in_paragraph = true;
        self.in_text = false;
        self.pending_text.clear();
        self.paragraph_started_emitted = false;
        self.pending_paragraph_alignment = None;
        self.pending_paragraph_classification = None;
        self.current_paragraph_block = ParagraphBlockKind::Paragraph;
    }

    /// Queues the appropriate Start event once paragraph properties have been parsed.
    fn ensure_paragraph_started(&mut self) {
        if self.in_paragraph && !self.paragraph_started_emitted {
            let kind = match self.pending_paragraph_classification.take() {
                Some(crate::styles::StyleClassification::Heading { level }) => {
                    ParagraphBlockKind::Heading { level }
                }
                Some(crate::styles::StyleClassification::BlockQuote) => {
                    ParagraphBlockKind::BlockQuote
                }
                Some(crate::styles::StyleClassification::Code) => ParagraphBlockKind::Preformatted,
                _ => ParagraphBlockKind::Paragraph,
            };
            self.current_paragraph_block = kind;
            let event = match kind {
                ParagraphBlockKind::Paragraph => Event::StartParagraph {
                    alignment: self.pending_paragraph_alignment.clone(),
                    id: None,
                },
                ParagraphBlockKind::Heading { level } => Event::StartHeading { level, id: None },
                ParagraphBlockKind::BlockQuote => Event::StartBlockQuote { id: None },
                ParagraphBlockKind::Preformatted => Event::StartPreformatted {
                    id: None,
                    syntax: None,
                },
            };
            self.queue.push_back(event);
            self.paragraph_started_emitted = true;
        }
    }
    /// Returns the next parsed `DocSpec` event from `document.xml`.
    #[inline]
    pub fn next_event(&mut self) -> Result<Option<Event>> {
        loop {
            if let Some(event) = self.queue.pop_front() {
                return Ok(Some(event));
            }

            match self.phase {
                Phase::NotStarted => {
                    self.phase = Phase::Running;
                    self.queue.push_back(Event::StartDocument {
                        id: None,
                        language: None,
                        metadata: None,
                    });
                }
                Phase::Finished => return Ok(None),
                Phase::Running => self.read_until_event()?,
            }
        }
    }
}

fn is_ignored_container(local: &[u8]) -> bool {
    matches!(
        local,
        b"sdt"
            | b"hyperlink"
            | b"drawing"
            | b"pict"
            | b"object"
            | b"ins"
            | b"del"
            | b"moveFrom"
            | b"moveTo"
            | b"tblPr"
            | b"trPr"
            | b"tcPr"
            | b"tblGrid"
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
    #![allow(clippy::expect_used, clippy::panic)]
    use std::io::{Cursor, Read};

    use super::*;

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
        DocxData { style_list }
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
        };
        DocumentReader::from_xml_reader(xml, data)
    }

    #[test]
    fn queue_length_never_exceeds_sixteen() -> core::result::Result<(), Box<dyn core::error::Error>>
    {
        let doc = {
            let mut content = String::from(
                r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
            );
            for _ in 0..1000 {
                content.push_str("<w:p><w:r><w:t>hello</w:t></w:r></w:p>");
            }
            content.push_str("</w:body></w:document>");
            content
        };
        let mut reader = make_reader(&doc);
        loop {
            if reader.queue.len() > 16 {
                return Err(Box::new(Error::Other {
                    message: format!("queue grew to {}", reader.queue.len()),
                }));
            }
            if reader.next_event()?.is_none() {
                break;
            }
        }
        Ok(())
    }

    #[test]
    fn queue_length_remains_bounded_with_colors(
    ) -> core::result::Result<(), Box<dyn core::error::Error>> {
        let doc = {
            let mut content = String::from(
                r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
            );
            for _ in 0..1000 {
                content.push_str(
                    r#"<w:p><w:r><w:rPr><w:b/><w:color w:val="FF0000"/><w:highlight w:val="yellow"/><w:shd w:val="clear" w:fill="0000FF"/></w:rPr><w:t>hello</w:t></w:r></w:p>"#,
                );
            }
            content.push_str("</w:body></w:document>");
            content
        };
        let mut reader = make_reader(&doc);
        loop {
            if reader.queue.len() > 32 {
                return Err(Box::new(Error::Other {
                    message: format!("queue grew to {}", reader.queue.len()),
                }));
            }
            if reader.next_event()?.is_none() {
                break;
            }
        }
        Ok(())
    }

    #[test]
    fn buf_is_cleared_per_iteration() -> core::result::Result<(), Box<dyn core::error::Error>> {
        let doc = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#;
        let mut reader = make_reader(doc);
        while reader.next_event()?.is_some() {
            if !reader.buf.is_empty() {
                return Err(Box::new(Error::Other {
                    message: "buf not cleared after event".to_string(),
                }));
            }
        }
        Ok(())
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
            <w:name w:val="Source Code"/>
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
            <w:name w:val="Source Code"/>
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
            <w:name w:val="Source Code"/>
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
}
