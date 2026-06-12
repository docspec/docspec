//! DOCX main document part (`document.xml`) streaming event parser.

use alloc::collections::VecDeque;
use core::fmt;
use std::io::{BufReader, Read};
use std::sync::{Arc, Mutex};

use docspec_core::{
    Color, Error, Event, ImageSource, Result, TableHeaderScope, TextAlignment, TextStyleKind,
};
use quick_xml::events::{BytesCData, BytesRef, BytesStart, BytesText};

use crate::properties;
use crate::rels::{HyperlinkMap, ImageMap};
use crate::styles::StyleList;

const MAX_LIST_LEVEL: u32 = 8;

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

/// One open level on the list nesting stack.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ListStackEntry {
    /// The w:numId of the list this entry belongs to.
    num_id: u32,
    /// The 0-indexed nesting depth (w:ilvl).
    ilvl: u32,
    /// Whether this entry's level is ordered (true) or unordered (false).
    is_ordered: bool,
}

#[derive(Default)]
struct DrawingScanState {
    pending_alt: Option<String>,
    current_pic_alt: Option<String>,
    pic_depth: u32,
    blip_fill_depth: u32,
    emitted_for_current_pic: bool,
}

/// Bundle of package-level data the document reader consults during streaming.
///
/// Forward-compatible: additional package parts (theme, settings)
/// will be added as fields here without breaking the constructor signature.
pub struct DocxData {
    /// Styles loaded from the styles part, used to classify paragraph and run styles.
    pub style_list: StyleList,
    /// Map of relationship Id → Target URL for every <w:hyperlink> r:id reference. Resolved from word/_rels/document.xml.rels at package-open time; empty if the rels file is absent or contains no hyperlink relationships.
    pub hyperlink_map: HyperlinkMap,
    /// Numbering definitions loaded from the numbering part, used to resolve list styles.
    pub numbering: crate::numbering::MinimalNumbering,
    /// Map of relationship Id → [`crate::rels::ImageRel`] for every image relationship in the document part. Resolved from word/_rels/document.xml.rels at package-open time; empty if absent.
    pub image_map: ImageMap,
}

/// Deferred-emission state for an open <w:hyperlink>.
///
/// Populated when `handle_start` or `handle_empty` sees a hyperlink whose
/// `r:id` resolves OR whose `w:anchor` is non-empty. Flushed to the event
/// queue as `Event::StartLink` immediately before the first inline
/// content event (`Text`, `LineBreak`, `StartTextStyle`) inside the link.
/// Discarded without emission if the hyperlink closes empty.
#[derive(Debug, Clone)]
pub(crate) struct PendingLink {
    /// Resolved URL or "#anchor". Never empty.
    href: String,
    /// Optional XML-decoded tooltip text.
    title: Option<String>,
    /// True after `StartLink` has been emitted to the queue.
    /// Used to decide whether `EndLink` should be emitted when the
    /// hyperlink closes (or paragraph/EOF auto-closes).
    link_started: bool,
}

#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DeniedKind {
    // 9 doc-level denied containers.
    Pict,
    Object,
    Del,
    MoveFrom,
    TblPr,
    TblGrid,
    TblPrEx,
    SdtPr,
    SdtEndPr,
    // 4 one-shot suppression markers.
    PPr,
    RPr,
    TrPr,
    TcPr,
}

/// Streaming parser for the DOCX main document XML part.
#[expect(
    clippy::struct_excessive_bools,
    reason = "DocumentReader tracks independent boolean parser states; grouping them would obscure the streaming state machine"
)]
pub struct DocumentReader {
    /// Reusable buffer for quick-xml event reading.
    buf: Vec<u8>,
    /// Stack of denied subtrees and one-shot suppression markers.
    denied_stack: Vec<DeniedKind>,
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
    /// Symbol font accumulated while inside `<w:rPr>`.
    pending_run_font: Option<crate::symbol_fonts::SymbolFont>,
    /// Symbol font frozen at `</w:rPr>`, applied to subsequent text in the same run.
    frozen_run_font: Option<crate::symbol_fonts::SymbolFont>,
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
    /// Hyperlink relationship map from the package. Consulted in `handle_start`
    /// when processing `<w:hyperlink r:id="...">` to resolve the URL.
    hyperlink_map: HyperlinkMap,
    /// Whether currently inside a `<w:tcPr>` element that is still legal (first child of cell).
    in_tcpr: bool,
    /// Whether currently inside a `<w:trPr>` element that is still legal (first child of row).
    in_trpr: bool,
    /// Colspan captured from `<w:gridSpan>` while inside `<w:tcPr>`.
    pending_colspan: Option<u32>,
    /// True once `StartTableCell` or `StartTableHeader` has been queued for the current cell.
    cell_started_emitted: bool,
    /// Whether currently inside a `<w:tc>` element.
    in_table_cell: bool,
    /// True when the most recent cell was emitted as `StartTableHeader` (so `</w:tc>` emits `EndTableHeader`).
    current_cell_is_header: bool,
    /// True when `<w:tblHeader/>` (truthy `CT_OnOff`) has been seen in the current `<w:trPr>`.
    pending_row_is_header: bool,
    /// True once `StartTableRow` has been queued for the current row.
    row_started_emitted: bool,
    /// Stack of outer row state saved while parsing rows inside nested tables.
    /// Tuple order: `pending_row_is_header`, `row_started_emitted`, `in_trpr`.
    nested_row_state_stack: Vec<(bool, bool, bool)>,
    /// Count of currently-open `<w:tbl>` elements (for nested-table depth tracking).
    table_depth: u32,
    /// True while the contiguous header band at the top of the outermost table is still open.
    header_band_open: bool,
    /// Nesting depth for `<w:hyperlink>`. Incremented on hyperlink open
    /// (only when `denied_stack` is empty). Decremented on hyperlink close.
    /// `StartLink` is emitted only at the 0→1 transition; `EndLink` only at 1→0.
    hyperlink_depth: u32,
    /// State of the currently-open (depth >= 1) hyperlink, if any.
    /// Some when a hyperlink has been opened but possibly before its
    /// `StartLink` has been flushed. None when no hyperlink is open.
    pending_link: Option<PendingLink>,
    /// Open list nesting stack. Each entry represents one open list level.
    list_stack: Vec<ListStackEntry>,
    /// Set of numIds whose first item has been emitted document-wide.
    /// Persists across non-list paragraph breaks.
    seen_lists: std::collections::HashSet<u32>,
    /// (numId, ilvl) captured from `<w:numPr>` during `<w:pPr>` parsing.
    /// Consumed by `ensure_paragraph_started`.
    pending_paragraph_list: Option<(u32, u32)>,
    /// True while inside `<w:numPr>` and capturing children.
    /// Mirrors the `in_ppr` field pattern.
    in_numpr: bool,
    /// w:numId captured from `<w:numId w:val="..."/>` inside `<w:numPr>` (None until seen).
    pending_num_pr_id: Option<u32>,
    /// w:ilvl captured from `<w:ilvl w:val="..."/>` inside `<w:numPr>` (None until seen).
    pending_num_pr_ilvl: Option<u32>,
    /// The quick-xml reader streaming from the document entry.
    xml: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>,
    /// Shared DOCX ZIP archive — used to stream embedded asset bytes on demand.
    archive: Arc<Mutex<zip::ZipArchive<Box<dyn crate::package::ReadSeek + 'static>>>>,
    /// Content-type lookup table — used by `DocxAssetHandle` to resolve MIME types.
    content_types: Arc<crate::content_types::ContentTypes>,
}

impl fmt::Debug for DocumentReader {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pending_link = self
            .pending_link
            .as_ref()
            .map(|pending| (&pending.href, &pending.title, pending.link_started));
        let mut debug = f.debug_struct("DocumentReader");
        debug
            .field("buf", &self.buf)
            .field("denied_stack", &self.denied_stack)
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
            .field("pending_run_font", &self.pending_run_font)
            .field("frozen_run_font", &self.frozen_run_font)
            .field("open_styles", &self.open_styles)
            .field("phase", &"<phase>")
            .field("queue", &self.queue)
            .field("run_content_emitted", &self.run_content_emitted)
            .field("data", &"<DocxData>")
            .field("hyperlink_map", &self.hyperlink_map)
            .field("hyperlink_depth", &self.hyperlink_depth)
            .field("pending_link", &pending_link)
            .field("list_stack", &self.list_stack)
            .field("seen_lists", &self.seen_lists)
            .field("pending_paragraph_list", &self.pending_paragraph_list)
            .field("in_numpr", &self.in_numpr)
            .field("pending_num_pr_id", &self.pending_num_pr_id)
            .field("pending_num_pr_ilvl", &self.pending_num_pr_ilvl);
        if std::env::var_os("DOCSPEC_DEBUG_DEFERRED_TABLE_SCAFFOLD").is_some() {
            debug
                .field("in_tcpr", &self.in_tcpr)
                .field("in_trpr", &self.in_trpr)
                .field("pending_colspan", &self.pending_colspan)
                .field("cell_started_emitted", &self.cell_started_emitted)
                .field("in_table_cell", &self.in_table_cell)
                .field("current_cell_is_header", &self.current_cell_is_header)
                .field("pending_row_is_header", &self.pending_row_is_header)
                .field("row_started_emitted", &self.row_started_emitted)
                .field("nested_row_state_stack", &self.nested_row_state_stack)
                .field("table_depth", &self.table_depth)
                .field("header_band_open", &self.header_band_open);
        }
        debug.field("xml", &"<quick_xml::Reader>");
        debug.field("archive", &"<Arc<Mutex<ZipArchive>>>");
        debug.field("content_types", &"<Arc<ContentTypes>>");
        debug.finish()
    }
}

impl DocumentReader {
    pub fn from_xml_reader_and_archive(
        mut xml: quick_xml::Reader<BufReader<Box<dyn Read + Send>>>,
        data: DocxData,
        archive: Arc<Mutex<zip::ZipArchive<Box<dyn crate::package::ReadSeek + 'static>>>>,
        content_types: Arc<crate::content_types::ContentTypes>,
    ) -> Self {
        xml.config_mut().check_end_names = false;
        let DocxData {
            style_list,
            hyperlink_map,
            numbering,
            image_map,
        } = data;
        Self {
            buf: Vec::with_capacity(4096),
            denied_stack: Vec::new(),
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
            pending_run_font: None,
            frozen_run_font: None,
            open_styles: Vec::new(),
            phase: Phase::NotStarted,
            queue: VecDeque::new(),
            run_content_emitted: false,
            data: DocxData {
                style_list,
                hyperlink_map: HyperlinkMap::default(),
                numbering,
                image_map,
            },
            hyperlink_map,
            in_tcpr: false,
            in_trpr: false,
            pending_colspan: None,
            cell_started_emitted: false,
            in_table_cell: false,
            current_cell_is_header: false,
            pending_row_is_header: false,
            row_started_emitted: false,
            nested_row_state_stack: Vec::new(),
            table_depth: 0,
            header_band_open: false,
            hyperlink_depth: 0,
            pending_link: None,
            list_stack: Vec::new(),
            seen_lists: std::collections::HashSet::new(),
            pending_paragraph_list: None,
            in_numpr: false,
            pending_num_pr_id: None,
            pending_num_pr_ilvl: None,
            xml,
            archive,
            content_types,
        }
    }

    #[cfg(test)]
    #[cfg(test)]
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
    fn can_collect_text(&self) -> bool {
        self.denied_stack.is_empty() && self.in_paragraph && self.in_text
    }

    fn emit_line_break(&mut self) {
        self.ensure_paragraph_started();
        self.flush_pending_text();
        self.flush_pending_link_start();
        self.emit_deferred_starts();
        self.run_content_emitted = true;
        self.queue.push_back(Event::LineBreak);
    }

    fn emit_tab(&mut self) {
        self.ensure_paragraph_started();
        self.flush_pending_text();
        self.flush_pending_link_start();
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
        self.pending_run_font = None;
        self.frozen_run_font = None;
        self.pending_run_text_color = None;
        self.pending_run_mark = None;
        self.pending_run_shade = None;
        if let Some(link) = self.pending_link.take() {
            if link.link_started {
                self.queue.push_back(Event::EndLink);
            }
        }
        self.hyperlink_depth = 0;
        let end_event = match &self.current_paragraph_block {
            ParagraphBlockKind::Paragraph
            | ParagraphBlockKind::OrderedListItem { .. }
            | ParagraphBlockKind::UnorderedListItem { .. } => Event::EndParagraph,
            ParagraphBlockKind::Heading { .. } => Event::EndHeading,
            ParagraphBlockKind::BlockQuote => Event::EndBlockQuote,
            ParagraphBlockKind::Preformatted => Event::EndPreformatted,
        };
        self.queue.push_back(end_event);
        self.in_paragraph = false;
        self.in_text = false;
        self.pending_text.clear();
        self.in_ppr = false;
        self.in_numpr = false;
        self.pending_num_pr_id = None;
        self.pending_num_pr_ilvl = None;
        self.pending_paragraph_alignment = None;
        self.pending_paragraph_classification = None;
        self.current_paragraph_block = ParagraphBlockKind::Paragraph;
        self.paragraph_started_emitted = false;
    }

    fn flush_list_stack(&mut self) {
        while let Some(entry) = self.list_stack.pop() {
            let event = if entry.is_ordered {
                Event::EndOrderedListItem
            } else {
                Event::EndUnorderedListItem
            };
            self.queue.push_back(event);
        }
    }

    fn compute_start(&mut self, num_id: u32) -> Option<u64> {
        self.seen_lists.insert(num_id).then_some(1)
    }

    fn reconcile_list_stack(&mut self, num_id: u32, ilvl: u32, is_ordered: bool) {
        while let Some(top) = self.list_stack.last().copied() {
            match top.ilvl.cmp(&ilvl) {
                core::cmp::Ordering::Greater => {
                    self.list_stack.pop();
                    let end = if top.is_ordered {
                        Event::EndOrderedListItem
                    } else {
                        Event::EndUnorderedListItem
                    };
                    self.queue.push_back(end);
                }
                core::cmp::Ordering::Equal => {
                    self.list_stack.pop();
                    let end = if top.is_ordered {
                        Event::EndOrderedListItem
                    } else {
                        Event::EndUnorderedListItem
                    };
                    self.queue.push_back(end);
                    break;
                }
                core::cmp::Ordering::Less => break,
            }
        }

        let target_depth = usize::try_from(ilvl).unwrap_or(usize::MAX);
        while self.list_stack.len() < target_depth {
            let phantom_ilvl = u32::try_from(self.list_stack.len()).unwrap_or(u32::MAX);
            let phantom_style = if is_ordered {
                docspec_core::ListStyleType::Decimal
            } else {
                docspec_core::ListStyleType::Disc
            };
            let start_event = if is_ordered {
                Event::StartOrderedListItem {
                    id: Some(num_id.to_string()),
                    level: phantom_ilvl,
                    start: None,
                    style_type: phantom_style,
                }
            } else {
                Event::StartUnorderedListItem {
                    id: Some(num_id.to_string()),
                    level: phantom_ilvl,
                    style_type: phantom_style,
                }
            };
            self.list_stack.push(ListStackEntry {
                num_id,
                ilvl: phantom_ilvl,
                is_ordered,
            });
            self.queue.push_back(start_event);
        }

        self.list_stack.push(ListStackEntry {
            num_id,
            ilvl,
            is_ordered,
        });
    }

    fn flush_pending_text(&mut self) {
        if self.pending_text.is_empty() {
            return;
        }

        let content = if let Some(font) = self.frozen_run_font {
            let mut out = String::with_capacity(self.pending_text.len());
            for ch in self.pending_text.chars() {
                let key = match u32::from(ch) {
                    cp @ 0xF020..=0xF0FF => cp
                        .checked_sub(0xF000)
                        .and_then(|stripped| u8::try_from(stripped).ok()),
                    cp @ 0x0020..=0x00FF => u8::try_from(cp).ok(),
                    _ => None,
                };
                if let Some(k) = key {
                    if let Some(mapped) = font.convert(k) {
                        out.push(mapped);
                    }
                }
            }
            self.pending_text.clear();
            out
        } else {
            core::mem::take(&mut self.pending_text)
        };

        if !content.is_empty() {
            self.flush_pending_link_start();
            self.emit_deferred_starts();
            self.queue.push_back(Event::Text { content });
        }
    }

    fn flush_pending_link_start(&mut self) {
        let should_start = self
            .pending_link
            .as_ref()
            .is_some_and(|link| !link.link_started);
        if !should_start {
            return;
        }

        self.ensure_cell_started();
        self.ensure_paragraph_started();

        if let Some(link) = self.pending_link.as_mut() {
            if !link.link_started {
                let href = link.href.clone();
                let title = link.title.clone();
                link.link_started = true;
                self.queue.push_back(Event::StartLink {
                    href,
                    id: None,
                    title,
                });
            }
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

    fn handle_rpr_property(&mut self, local: &[u8], tag: &BytesStart<'_>) -> bool {
        if !self.in_rpr {
            return false;
        }
        match local {
            b"b" => {
                self.set_pending_run_kind(TextStyleKind::Bold, parse_on_off_attribute(tag));
            }
            b"i" => {
                self.set_pending_run_kind(TextStyleKind::Italic, parse_on_off_attribute(tag));
            }
            b"strike" | b"dstrike" => {
                self.set_pending_run_kind(
                    TextStyleKind::Strikethrough,
                    parse_on_off_attribute(tag),
                );
            }
            b"u" => {
                let val = read_val_attribute(tag);
                self.set_pending_run_kind(
                    TextStyleKind::Underline,
                    properties::parse_underline_on(val.as_deref()),
                );
            }
            b"vertAlign" => {
                let val = read_val_attribute(tag);
                self.set_pending_vertical_alignment(properties::parse_vert_align(val.as_deref()));
            }
            b"color" => {
                let val = read_val_attribute(tag);
                self.pending_run_text_color = properties::parse_color_val(val.as_deref());
            }
            b"highlight" => {
                let val = read_val_attribute(tag);
                self.pending_run_mark = properties::parse_highlight_val(val.as_deref());
            }
            b"shd" => {
                let fill = read_attribute(tag, b"w:fill");
                self.pending_run_shade = properties::parse_shd_fill(fill.as_deref());
            }
            b"rFonts" => {
                self.pending_run_font = read_rfonts_symbol(tag);
            }
            b"rStyle" => self.handle_rpr_rstyle(tag),
            _ => return false,
        }
        true
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
        if !self.denied_stack.is_empty() || is_denied_container(local).is_some() {
            return;
        }
        if self.handle_rpr_property(local, tag) {
            return;
        }
        match local {
            value if !self.denied_stack.is_empty() || is_denied_container(value).is_some() => {}
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
            b"gridSpan" if self.in_tcpr => {
                let val = read_val_attribute(tag);
                self.pending_colspan = properties::parse_grid_span_value(val.as_deref());
            }
            b"tblHeader" if self.in_trpr => {
                self.pending_row_is_header =
                    properties::parse_on_off(read_val_attribute(tag).as_deref());
            }
            b"numId" if self.in_numpr => {
                if let Some(val) = read_val_attribute(tag) {
                    if let Ok(n) = val.parse::<u32>() {
                        self.pending_num_pr_id = Some(n);
                    }
                }
            }
            b"ilvl" if self.in_numpr => {
                if let Some(val) = read_val_attribute(tag) {
                    if let Ok(n) = val.parse::<u32>() {
                        self.pending_num_pr_ilvl = Some(n);
                    }
                }
            }
            b"vMerge" if self.in_tcpr => {
                // vMerge (rowspan) deferred — requires event model change or table buffering, out of scope
            }
            b"rPr" if self.in_ppr => {}
            b"rPr" if self.in_paragraph && !self.in_ppr && !self.in_rpr => {}
            b"sym" if self.in_paragraph && !self.in_rpr => {
                let font_name = read_attribute(tag, b"w:font");
                let char_hex = read_attribute(tag, b"w:char");
                if let (Some(name), Some(hex)) = (font_name, char_hex) {
                    if let (Some(font), Some(key)) = (
                        crate::symbol_fonts::SymbolFont::from_name(&name),
                        crate::properties::parse_sym_char(&hex),
                    ) {
                        if let Some(ch) = font.convert(key) {
                            self.flush_pending_text();
                            self.ensure_paragraph_started();
                            self.emit_deferred_starts();
                            self.queue.push_back(Event::Text {
                                content: String::from(ch),
                            });
                            self.run_content_emitted = true;
                        }
                    }
                }
            }
            b"p" if !self.in_paragraph => {
                self.ensure_cell_started();
                self.start_paragraph();
                self.end_paragraph();
            }
            b"br" if self.in_paragraph => {
                self.ensure_cell_started();
                self.emit_line_break();
            }
            b"tab" if self.in_paragraph => {
                self.ensure_cell_started();
                self.emit_tab();
            }
            b"hyperlink" => Self::handle_empty_hyperlink(),
            _ => {}
        }
    }

    fn handle_empty_hyperlink() {}

    fn handle_end(&mut self, local: &[u8]) {
        if let Some(&top) = self.denied_stack.last() {
            if denied_kind_for(local) == Some(top) {
                self.denied_stack.pop();
            }
            return;
        }

        match local {
            b"p" if self.in_paragraph => self.end_paragraph(),
            b"numPr" if self.in_numpr => {
                if let Some(num_id) = self.pending_num_pr_id {
                    let ilvl = self.pending_num_pr_ilvl.unwrap_or(0);
                    self.pending_paragraph_list = Some((num_id, ilvl));
                }
                self.in_numpr = false;
                self.pending_num_pr_id = None;
                self.pending_num_pr_ilvl = None;
            }
            b"pPr" if self.in_ppr => {
                self.ensure_paragraph_started();
                self.in_ppr = false;
            }
            b"tcPr" if self.in_tcpr => {
                self.in_tcpr = false;
                self.ensure_cell_started();
            }
            b"trPr" if self.in_trpr => {
                self.in_trpr = false;
            }
            b"rPr" if self.in_rpr => {
                self.frozen_run_kinds = core::mem::take(&mut self.pending_run_kinds);
                self.frozen_run_text_color = self.pending_run_text_color.take();
                self.frozen_run_mark = self
                    .pending_run_mark
                    .take()
                    .or_else(|| self.pending_run_shade.take());
                self.frozen_run_font = self.pending_run_font.take();
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
                self.pending_run_font = None;
                self.frozen_run_font = None;
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
            b"tbl" => {
                self.flush_list_stack();
                self.table_depth = self.table_depth.saturating_sub(1);
                self.queue.push_back(Event::EndTable);
            }
            b"tr" => {
                self.ensure_row_started();
                self.queue.push_back(Event::EndTableRow);
                if self.table_depth > 1 {
                    if let Some((pending_row_is_header, row_started_emitted, in_trpr)) =
                        self.nested_row_state_stack.pop()
                    {
                        self.pending_row_is_header = pending_row_is_header;
                        self.row_started_emitted = row_started_emitted;
                        self.in_trpr = in_trpr;
                    }
                }
            }
            b"tc" => {
                self.ensure_cell_started();
                self.flush_list_stack();
                if self.current_cell_is_header && self.table_depth == 1 {
                    self.queue.push_back(Event::EndTableHeader);
                } else {
                    self.queue.push_back(Event::EndTableCell);
                }
                self.in_table_cell = false;
            }
            b"hyperlink" if self.hyperlink_depth > 0 => {
                if self.hyperlink_depth == 1 {
                    if let Some(link) = self.pending_link.take() {
                        if link.link_started {
                            self.queue.push_back(Event::EndLink);
                        }
                    }
                }
                self.hyperlink_depth = self.hyperlink_depth.saturating_sub(1);
            }
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
        self.flush_list_stack();
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

    fn handle_start(&mut self, tag: &BytesStart<'_>) -> Result<()> {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        if !self.denied_stack.is_empty() {
            if let Some(kind) = is_denied_container(local) {
                self.denied_stack.push(kind);
            }
            return Ok(());
        }
        if local == b"drawing" {
            self.handle_drawing_start()?;
            return Ok(());
        }
        if self.handle_table_start(local, tag) {
            return Ok(());
        }
        if self.handle_rpr_property(local, tag) {
            return Ok(());
        }

        let denied_container = is_denied_container(local);
        match (local, denied_container) {
            (_, Some(kind)) => self.denied_stack.push(kind),
            (b"pPr", _) if self.in_paragraph => {
                if self.paragraph_started_emitted {
                    // Out-of-order pPr: StartParagraph already emitted; silently consume
                    self.denied_stack.push(DeniedKind::PPr);
                } else {
                    self.in_ppr = true;
                    self.pending_paragraph_alignment = None;
                }
            }
            (b"jc", _) if self.in_ppr => {
                let val = read_val_attribute(tag);
                self.pending_paragraph_alignment =
                    val.as_deref().and_then(properties::parse_alignment);
            }
            (b"pStyle", _) if self.in_ppr && !self.paragraph_started_emitted => {
                self.pending_paragraph_classification = read_val_attribute(tag)
                    .filter(|s| !s.is_empty())
                    .and_then(|s| self.data.style_list.classify(&s));
            }
            (b"numPr", _) if self.in_ppr && !self.paragraph_started_emitted => {
                self.in_numpr = true;
                self.pending_num_pr_id = None;
                self.pending_num_pr_ilvl = None;
            }
            (b"rPr", _) if self.in_ppr => {
                self.denied_stack.push(DeniedKind::RPr);
            }
            (b"rPr", _) if self.in_paragraph && !self.in_ppr && !self.in_rpr => {
                if self.run_content_emitted {
                    // Out-of-order rPr: content already emitted in this run; silently consume
                    self.denied_stack.push(DeniedKind::RPr);
                } else {
                    self.in_rpr = true;
                    self.pending_run_kinds.clear();
                    self.pending_run_text_color = None;
                    self.pending_run_mark = None;
                    self.pending_run_shade = None;
                    self.pending_run_font = None;
                }
            }
            (b"p", _) if !self.in_paragraph => {
                self.ensure_cell_started();
                self.start_paragraph();
            }
            (b"r", _) if self.in_paragraph => {
                self.ensure_cell_started();
                self.ensure_paragraph_started();
            }
            (b"t", _) if self.in_paragraph => {
                self.ensure_cell_started();
                self.ensure_paragraph_started();
                self.in_text = true;
                self.pending_text.clear();
                self.run_content_emitted = true;
            }
            (b"br", _) if self.in_paragraph => {
                self.ensure_cell_started();
                self.emit_line_break();
            }
            (b"tab", _) if self.in_paragraph => {
                self.ensure_cell_started();
                self.emit_tab();
            }
            (b"hyperlink", _) if !self.in_paragraph => {
                self.hyperlink_depth = self.hyperlink_depth.saturating_add(1);
            }
            (b"hyperlink", _) => self.handle_hyperlink_start(tag),
            _ => {}
        }
        Ok(())
    }

    fn handle_drawing_start(&mut self) -> Result<()> {
        if self.in_paragraph {
            self.ensure_cell_started();
            self.ensure_paragraph_started();
            self.flush_pending_text();
            self.flush_pending_link_start();
        }
        self.run_content_emitted = true;
        self.parse_drawing_subtree()
    }

    fn parse_drawing_subtree(&mut self) -> Result<()> {
        let mut state = DrawingScanState::default();
        let mut drawing_depth: u32 = 1;

        while drawing_depth > 0 {
            let event = self
                .xml
                .read_event_into(&mut self.buf)
                .map_err(map_quick_xml_error)?
                .into_owned();

            match event {
                quick_xml::events::Event::Start(tag) => {
                    self.handle_drawing_child_start(&mut state, &tag);
                    drawing_depth = drawing_depth.saturating_add(1);
                }
                quick_xml::events::Event::Empty(tag) => {
                    self.handle_drawing_child_empty(&mut state, &tag);
                }
                quick_xml::events::Event::End(tag) => {
                    if tag.local_name().as_ref() == b"drawing" && drawing_depth == 1 {
                        drawing_depth = drawing_depth.saturating_sub(1);
                    } else {
                        Self::handle_drawing_child_end(&mut state, tag.local_name().as_ref());
                        drawing_depth = drawing_depth.saturating_sub(1);
                    }
                }
                quick_xml::events::Event::Eof => {
                    self.handle_eof();
                    drawing_depth = 0;
                }
                quick_xml::events::Event::Text(_)
                | quick_xml::events::Event::GeneralRef(_)
                | quick_xml::events::Event::CData(_)
                | quick_xml::events::Event::Comment(_)
                | quick_xml::events::Event::Decl(_)
                | quick_xml::events::Event::PI(_)
                | quick_xml::events::Event::DocType(_) => {}
            }
            self.buf.clear();
        }

        Ok(())
    }

    fn handle_drawing_child_start(&mut self, state: &mut DrawingScanState, tag: &BytesStart<'_>) {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        match local {
            b"docPr" => {
                state.pending_alt = read_decoded_attribute(tag, b"descr");
            }
            b"pic" => {
                state.pic_depth = state.pic_depth.saturating_add(1);
                if state.pic_depth == 1 {
                    state.current_pic_alt = state.pending_alt.take();
                    state.emitted_for_current_pic = false;
                }
            }
            b"blipFill" if state.pic_depth > 0 => {
                state.blip_fill_depth = state.blip_fill_depth.saturating_add(1);
            }
            b"blip" => self.maybe_emit_drawing_blip(state, tag),
            _ => {}
        }
    }

    fn handle_drawing_child_empty(&mut self, state: &mut DrawingScanState, tag: &BytesStart<'_>) {
        let local_name = tag.local_name();
        let local = local_name.as_ref();
        match local {
            b"docPr" => {
                state.pending_alt = read_decoded_attribute(tag, b"descr");
            }
            b"blip" => self.maybe_emit_drawing_blip(state, tag),
            _ => {}
        }
    }

    fn handle_drawing_child_end(state: &mut DrawingScanState, local: &[u8]) {
        match local {
            b"blipFill" if state.blip_fill_depth > 0 => {
                state.blip_fill_depth = state.blip_fill_depth.saturating_sub(1);
            }
            b"pic" if state.pic_depth > 0 => {
                state.pic_depth = state.pic_depth.saturating_sub(1);
                if state.pic_depth == 0 {
                    state.current_pic_alt = None;
                    state.emitted_for_current_pic = false;
                }
            }
            _ => {}
        }
    }

    fn maybe_emit_drawing_blip(&mut self, state: &mut DrawingScanState, tag: &BytesStart<'_>) {
        if state.pic_depth == 0 || state.blip_fill_depth == 0 || state.emitted_for_current_pic {
            return;
        }

        let embed = read_attribute(tag, b"r:embed");
        let link = read_attribute(tag, b"r:link");
        let Some(rid) = embed.or(link) else {
            return;
        };

        state.emitted_for_current_pic = true;
        self.queue.push_back(Event::Image {
            alt: state.current_pic_alt.clone(),
            decorative: false,
            id: None,
            source: self.image_source_for_rid(&rid),
            title: None,
        });
    }

    fn image_source_for_rid(&self, rid: &str) -> ImageSource {
        let asset_id = match self.data.image_map.get(rid) {
            Some(rel) if rel.is_external => {
                return ImageSource::Uri {
                    uri: rel.target.clone(),
                }
            }
            Some(rel) => format!("zip://{}", rel.target),
            None => rid.to_string(),
        };
        ImageSource::Asset(Arc::new(crate::asset_provider::DocxAssetHandle::new(
            Arc::clone(&self.archive),
            Arc::clone(&self.content_types),
            asset_id,
        )))
    }

    fn handle_hyperlink_start(&mut self, tag: &BytesStart<'_>) {
        self.hyperlink_depth = self.hyperlink_depth.saturating_add(1);
        if self.hyperlink_depth != 1 || self.pending_link.is_some() {
            return;
        }
        if self.current_paragraph_block == ParagraphBlockKind::Preformatted {
            return;
        }

        let rid = read_attribute(tag, b"r:id");
        let anchor = read_attribute(tag, b"w:anchor");
        let tooltip = read_attribute(tag, b"w:tooltip");

        let href = if let Some(rid_val) = rid {
            if let Some(target) = self.hyperlink_map.get(&rid_val) {
                target.clone()
            } else {
                return;
            }
        } else if let Some(anchor_val) = anchor.filter(|a| !a.is_empty()) {
            format!("#{anchor_val}")
        } else {
            return;
        };

        let title = tooltip.and_then(|t| {
            quick_xml::escape::unescape(&t)
                .ok()
                .map(std::borrow::Cow::into_owned)
        });

        self.pending_link = Some(PendingLink {
            href,
            title,
            link_started: false,
        });
    }

    fn handle_table_start(&mut self, local: &[u8], tag: &BytesStart<'_>) -> bool {
        match local {
            b"tbl" => {
                self.ensure_cell_started();
                self.flush_list_stack();
                self.table_depth = self.table_depth.saturating_add(1);
                if self.table_depth == 1 {
                    self.header_band_open = true;
                }
                self.queue.push_back(Event::StartTable { id: None });
                true
            }
            b"tr" => {
                if self.table_depth > 1 {
                    self.nested_row_state_stack.push((
                        self.pending_row_is_header,
                        self.row_started_emitted,
                        self.in_trpr,
                    ));
                }
                self.start_table_row();
                true
            }
            b"trPr" => {
                if self.row_started_emitted {
                    // Out-of-order trPr: row content already emitted; silently consume
                    self.denied_stack.push(DeniedKind::TrPr);
                } else {
                    self.in_trpr = true;
                }
                true
            }
            b"tblHeader" if self.in_trpr => {
                self.pending_row_is_header =
                    properties::parse_on_off(read_val_attribute(tag).as_deref());
                true
            }
            b"tc" => {
                self.start_table_cell();
                true
            }
            b"tcPr" => {
                if self.cell_started_emitted {
                    // Out-of-order tcPr: cell content already emitted; silently consume
                    self.denied_stack.push(DeniedKind::TcPr);
                } else {
                    self.in_tcpr = true;
                }
                true
            }
            b"gridSpan" if self.in_tcpr => {
                let val = read_val_attribute(tag);
                self.pending_colspan = properties::parse_grid_span_value(val.as_deref());
                true
            }
            b"vMerge" if self.in_tcpr => {
                // vMerge (rowspan) deferred — requires event model change or table buffering, out of scope
                true
            }
            _ => false,
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
            quick_xml::events::Event::Start(tag) => self.handle_start(&tag)?,
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
        self.pending_paragraph_list = None;
        self.current_paragraph_block = ParagraphBlockKind::Paragraph;
    }

    /// Queues the appropriate Start event once paragraph properties have been parsed.
    fn ensure_paragraph_started(&mut self) {
        if self.in_paragraph && !self.paragraph_started_emitted {
            let list_classification = match self.pending_paragraph_list.take() {
                None => None,
                Some((num_id, raw_ilvl)) => {
                    let ilvl = core::cmp::min(raw_ilvl, MAX_LIST_LEVEL);
                    let result = self.data.numbering.resolve(num_id, ilvl);
                    result
                        .is_list
                        .then_some((num_id, ilvl, result.is_ordered, result.style_type))
                }
            };

            let kind = match self.pending_paragraph_classification.take() {
                Some(crate::styles::StyleClassification::Heading { level }) => {
                    self.flush_list_stack();
                    ParagraphBlockKind::Heading { level }
                }
                Some(crate::styles::StyleClassification::BlockQuote) => {
                    self.flush_list_stack();
                    ParagraphBlockKind::BlockQuote
                }
                Some(crate::styles::StyleClassification::Code) => {
                    self.flush_list_stack();
                    ParagraphBlockKind::Preformatted
                }
                _ => match list_classification {
                    None => {
                        // Intentional: do NOT flush the list stack here. A plain paragraph
                        // following a list item attaches as continuation content of the
                        // open item. The list closes only at heading / block quote /
                        // preformatted / table boundary / cell boundary / end of document.
                        ParagraphBlockKind::Paragraph
                    }
                    Some((num_id, ilvl, is_ordered, style_type)) => {
                        self.reconcile_list_stack(num_id, ilvl, is_ordered);
                        let start = self.compute_start(num_id);
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
            self.current_paragraph_block = kind;
            self.emit_paragraph_start_for_current_block();
            self.paragraph_started_emitted = true;
        }
    }

    fn emit_paragraph_start_for_current_block(&mut self) {
        match &self.current_paragraph_block {
            ParagraphBlockKind::Paragraph => {
                self.queue.push_back(Event::StartParagraph {
                    alignment: self.pending_paragraph_alignment.clone(),
                    id: None,
                });
            }
            ParagraphBlockKind::Heading { level } => {
                self.queue.push_back(Event::StartHeading {
                    level: *level,
                    id: None,
                });
            }
            ParagraphBlockKind::BlockQuote => {
                self.queue.push_back(Event::StartBlockQuote { id: None });
            }
            ParagraphBlockKind::Preformatted => {
                self.queue.push_back(Event::StartPreformatted {
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
                self.queue.push_back(Event::StartOrderedListItem {
                    id: Some(num_id.to_string()),
                    level: *ilvl,
                    start: *start,
                    style_type: style_type.clone(),
                });
                self.queue.push_back(Event::StartParagraph {
                    alignment: self.pending_paragraph_alignment.clone(),
                    id: None,
                });
            }
            ParagraphBlockKind::UnorderedListItem {
                num_id,
                ilvl,
                style_type,
            } => {
                self.queue.push_back(Event::StartUnorderedListItem {
                    id: Some(num_id.to_string()),
                    level: *ilvl,
                    style_type: style_type.clone(),
                });
                self.queue.push_back(Event::StartParagraph {
                    alignment: self.pending_paragraph_alignment.clone(),
                    id: None,
                });
            }
        }
    }

    /// Resets cell state at the start of a new `<w:tc>` element.
    ///
    /// `current_cell_is_header` is only reset at the outermost table level. Inside a nested
    /// table, the outer cell's header flag must be preserved so that the outer `</w:tc>` emits
    /// the matching `EndTableHeader` (nested tables never emit headers — see `ensure_cell_started`).
    fn start_table_cell(&mut self) {
        self.cell_started_emitted = false;
        self.in_table_cell = true;
        if self.table_depth <= 1 {
            self.current_cell_is_header = false;
        }
        self.pending_colspan = None;
        self.in_tcpr = false;
    }

    /// Resets row state at the start of a new `<w:tr>` element.
    fn start_table_row(&mut self) {
        self.row_started_emitted = false;
        self.pending_row_is_header = false;
        self.in_trpr = false;
    }

    /// Queues `StartTableRow` once row properties have been parsed.
    ///
    /// Also implements the OOXML §17.4.49 contiguous-from-top rule: if this row is NOT a header
    /// row in the outermost table, the header band is permanently closed for the remainder of
    /// that table.
    fn ensure_row_started(&mut self) {
        if !self.row_started_emitted {
            // OOXML §17.4.49: close the header band on the first non-header row in the outermost table.
            if self.table_depth == 1 && !self.pending_row_is_header {
                self.header_band_open = false;
            }
            self.queue.push_back(Event::StartTableRow { id: None });
            self.row_started_emitted = true;
        }
    }

    /// Queues `StartTableCell` or `StartTableHeader` once cell properties have been parsed.
    ///
    /// The header decision uses: `pending_row_is_header && header_band_open && table_depth == 1`.
    fn ensure_cell_started(&mut self) {
        if self.in_table_cell && !self.cell_started_emitted {
            self.ensure_row_started();
            let is_header_cell =
                self.pending_row_is_header && self.header_band_open && self.table_depth == 1;
            if is_header_cell {
                self.queue.push_back(Event::StartTableHeader {
                    scope: Some(TableHeaderScope::Column),
                    abbr: None,
                    colspan: self.pending_colspan,
                    rowspan: None,
                    id: None,
                });
                self.current_cell_is_header = true;
            } else {
                self.queue.push_back(Event::StartTableCell {
                    colspan: self.pending_colspan,
                    rowspan: None,
                    id: None,
                });
            }
            self.cell_started_emitted = true;
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

/// Returns Some only for elements whose entire subtree should be silently dropped
/// when they appear at the document level. Does NOT include one-shot suppression
/// markers (PPr/RPr/TrPr/TcPr).
fn is_denied_container(local: &[u8]) -> Option<DeniedKind> {
    match local {
        b"pict" => Some(DeniedKind::Pict),
        b"object" => Some(DeniedKind::Object),
        b"del" => Some(DeniedKind::Del),
        b"moveFrom" => Some(DeniedKind::MoveFrom),
        b"tblPr" => Some(DeniedKind::TblPr),
        b"tblGrid" => Some(DeniedKind::TblGrid),
        b"tblPrEx" => Some(DeniedKind::TblPrEx),
        b"sdtPr" => Some(DeniedKind::SdtPr),
        b"sdtEndPr" => Some(DeniedKind::SdtEndPr),
        _ => None,
    }
}

/// Returns Some for any element that can ever be on the denied stack.
/// Superset of `is_denied_container`: adds PPr/RPr/TrPr/TcPr.
fn denied_kind_for(local: &[u8]) -> Option<DeniedKind> {
    match local {
        b"pict" => Some(DeniedKind::Pict),
        b"object" => Some(DeniedKind::Object),
        b"del" => Some(DeniedKind::Del),
        b"moveFrom" => Some(DeniedKind::MoveFrom),
        b"tblPr" => Some(DeniedKind::TblPr),
        b"tblGrid" => Some(DeniedKind::TblGrid),
        b"tblPrEx" => Some(DeniedKind::TblPrEx),
        b"sdtPr" => Some(DeniedKind::SdtPr),
        b"sdtEndPr" => Some(DeniedKind::SdtEndPr),
        b"pPr" => Some(DeniedKind::PPr),
        b"rPr" => Some(DeniedKind::RPr),
        b"trPr" => Some(DeniedKind::TrPr),
        b"tcPr" => Some(DeniedKind::TcPr),
        _ => None,
    }
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

fn map_quick_xml_error(err: quick_xml::Error) -> Error {
    match err {
        quick_xml::Error::Io(source) => Error::Io {
            source: std::io::Error::new(source.kind(), source.to_string()),
        },
        other => Error::Parse {
            message: format!("malformed document.xml: {other}"),
            position: None,
        },
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
    use core::fmt::Write as _;
    use std::io::{Cursor, Read};
    use std::sync::Arc;

    use docspec_core::{AssetHandle, ImageSource, ListStyleType};

    use super::*;

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

    #[test]
    fn document_reader_initializes_hyperlink_state_to_default() {
        let doc = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#;
        let reader = make_reader(doc);

        assert_eq!(reader.hyperlink_depth, 0);
        assert!(reader.pending_link.is_none());
    }

    #[test]
    fn document_reader_debug_includes_hyperlink_fields() {
        let doc = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#;
        let reader = make_reader(doc);
        let debug = format!("{reader:?}");

        assert!(debug.contains("hyperlink_depth"));
        assert!(debug.contains("pending_link"));
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
    fn queue_length_bounded_with_hyperlinks(
    ) -> core::result::Result<(), Box<dyn core::error::Error>> {
        let doc = {
            let mut content = String::from(
                r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>"#,
            );
            for i in 0..1000_u32 {
                write!(
                    content,
                    r#"<w:p><w:hyperlink r:id="rId{i}"><w:r><w:t>link{i}</w:t></w:r></w:hyperlink></w:p>"#
                )?;
            }
            content.push_str("</w:body></w:document>");
            content
        };
        let hyperlink_map: HyperlinkMap = (0..1000_u32)
            .map(|i| (format!("rId{i}"), format!("https://example.com/{i}")))
            .collect();
        let mut reader = make_reader_with_hyperlinks(&doc, hyperlink_map);
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
                    start: None,
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
                    start: None,
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
    fn non_list_paragraph_between_list_items_attaches_as_continuation() {
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
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "plain".to_string(),
                },
                docspec_core::Event::EndParagraph,
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
        assert_eq!(reader.pending_paragraph_list, None);
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
        assert_eq!(reader.pending_paragraph_list, None);
    }

    #[test]
    fn numpr_without_num_id_leaves_pending_paragraph_list_none() {
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
        assert_eq!(reader.pending_paragraph_list, None);
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
    fn multiple_continuation_paragraphs_attach_to_same_item() {
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
    fn trailing_continuation_paragraphs_close_with_list_at_document_end() {
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
                docspec_core::Event::EndOrderedListItem,
                docspec_core::Event::EndDocument,
            ]
        );
    }

    #[test]
    fn continuation_then_deeper_level_nests_under_continuation_owner() {
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
                    level: 1,
                    start: None,
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
    fn continuation_paragraph_inside_table_cell_attaches_then_cell_end_closes_list() {
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
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "continuation".to_string(),
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
                docspec_core::Event::StartParagraph {
                    alignment: None,
                    id: None,
                },
                docspec_core::Event::Text {
                    content: "An interruption in our regularly scheduled listing, for this essential and very relevant public service announcement.".to_string(),
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
}
