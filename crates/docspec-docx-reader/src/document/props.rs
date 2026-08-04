//! Property-container folds for runs, paragraphs, table cells, and table rows.
//!
//! Each `parse_*` here consumes exactly one OOXML property container —
//! `<w:rPr>`, `<w:pPr>`, `<w:numPr>`, `<w:tcPr>`, `<w:trPr>` — and returns a
//! plain resolved value. None of these containers nest document structure, so
//! each fold reads a bounded region and never recurses into the document tree.
//!
//! That bound is why they live apart from the reader: they need the cursor and
//! the package lookups, but nothing of the parser's descent or its output
//! queue, so they take only what they read.

use docspec_core::{Color, Result, TextAlignment, TextStyleKind};
use quick_xml::events::BytesStart;

use super::context::PackageContext;
use super::input::XmlCursor;
use super::{
    parse_error, parse_on_off_attribute, parse_u32_attr, read_attribute, read_rfonts_symbol,
    read_val_attribute,
};
use crate::properties;
use crate::styles::StyleClassification;
use crate::symbol_fonts::SymbolFont;

/// Run properties resolved from a single `<w:rPr>`.
pub(super) struct ResolvedRunProperties {
    /// Style kinds carrying no color, in application order.
    pub kinds: Vec<TextStyleKind>,
    /// Foreground color from `<w:color>`.
    pub text_color: Option<Color>,
    /// Highlight color, from `<w:highlight>` or the `<w:shd>` fallback.
    pub mark: Option<Color>,
    /// Symbol font to remap this run's text through.
    pub font: Option<SymbolFont>,
}

/// Paragraph properties resolved from a single `<w:pPr>`.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ResolvedParagraphProperties {
    /// Horizontal alignment from `<w:jc>`.
    pub alignment: Option<TextAlignment>,
    /// Classification of `<w:pStyle>` after following any `basedOn` chain.
    pub classification: Option<StyleClassification>,
    /// `(numId, ilvl)` from `<w:numPr>` when the paragraph is a list item.
    pub list_info: Option<(u32, u32)>,
}

/// Table-cell properties resolved from a single `<w:tcPr>`.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ResolvedCellProperties {
    /// Horizontal cell span from `<w:gridSpan>`.
    pub colspan: Option<u32>,
}

/// Table-row properties resolved from a single `<w:trPr>`.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ResolvedRowProperties {
    /// Whether `<w:tblHeader>` marks this row as a header row.
    pub is_header: bool,
}

/// Accumulator for run properties while `<w:rPr>` is being read.
///
/// Kept separate from [`ResolvedRunProperties`] because `<w:shd>` is only a
/// fallback: it must be held apart from `<w:highlight>` until the container
/// closes, since a later `<w:highlight>` outranks it regardless of order.
#[derive(Default)]
struct RunPropertyAccumulator {
    kinds: Vec<TextStyleKind>,
    text_color: Option<Color>,
    mark: Option<Color>,
    shade: Option<Color>,
    font: Option<SymbolFont>,
}

impl RunPropertyAccumulator {
    /// Collapses the accumulator, resolving highlight precedence over shading.
    fn finish(self) -> ResolvedRunProperties {
        ResolvedRunProperties {
            kinds: self.kinds,
            text_color: self.text_color,
            mark: self.mark.or(self.shade),
            font: self.font,
        }
    }
}

/// Sets or clears a style kind, keeping at most one occurrence of it.
fn set_resolved_run_kind(kinds: &mut Vec<TextStyleKind>, kind: TextStyleKind, enabled: bool) {
    kinds.retain(|current| current != &kind);
    if enabled {
        kinds.push(kind);
    }
}

/// Applies a vertical alignment, which is exclusive across sub- and superscript.
fn set_resolved_vertical_alignment(kinds: &mut Vec<TextStyleKind>, align: properties::VertAlign) {
    kinds.retain(|kind| kind != &TextStyleKind::Subscript && kind != &TextStyleKind::Superscript);
    match align {
        properties::VertAlign::Subscript => kinds.push(TextStyleKind::Subscript),
        properties::VertAlign::Superscript => kinds.push(TextStyleKind::Superscript),
        properties::VertAlign::None => {}
    }
}

/// Promotes the run to code style when `<w:rStyle>` resolves to a code style.
fn apply_resolved_rpr_style(
    package: &PackageContext,
    tag: &BytesStart<'_>,
    kinds: &mut Vec<TextStyleKind>,
) {
    if let Some(StyleClassification::Code) = read_val_attribute(tag)
        .filter(|s| !s.is_empty())
        .and_then(|s| package.classify_style(&s))
    {
        if !kinds.contains(&TextStyleKind::Code) {
            kinds.push(TextStyleKind::Code);
        }
    }
}

/// Folds one `<w:rPr>` child into the accumulator.
///
/// Returns whether the element was recognized.
fn apply_resolved_rpr_property(
    package: &PackageContext,
    local: &[u8],
    tag: &BytesStart<'_>,
    acc: &mut RunPropertyAccumulator,
) -> bool {
    match local {
        b"b" => {
            set_resolved_run_kind(
                &mut acc.kinds,
                TextStyleKind::Bold,
                parse_on_off_attribute(tag),
            );
        }
        b"i" => {
            set_resolved_run_kind(
                &mut acc.kinds,
                TextStyleKind::Italic,
                parse_on_off_attribute(tag),
            );
        }
        // <w:bCs> per ECMA-376 §17.3.2.2 and <w:iCs> per §17.3.2.21 apply only
        // to complex-script runs. DocSpec runs are not complex-script, so these
        // properties are silently ignored; §17.3.2.1 governs <w:b> and §17.3.2.20
        // governs <w:i> for non-complex-script text.
        b"bCs" | b"iCs" => {}
        b"strike" | b"dstrike" => {
            set_resolved_run_kind(
                &mut acc.kinds,
                TextStyleKind::Strikethrough,
                parse_on_off_attribute(tag),
            );
        }
        b"u" => {
            let val = read_val_attribute(tag);
            set_resolved_run_kind(
                &mut acc.kinds,
                TextStyleKind::Underline,
                properties::parse_underline_on(val.as_deref()),
            );
        }
        b"vertAlign" => {
            let val = read_val_attribute(tag);
            set_resolved_vertical_alignment(
                &mut acc.kinds,
                properties::parse_vert_align(val.as_deref()),
            );
        }
        b"color" => {
            let val = read_val_attribute(tag);
            acc.text_color = properties::parse_color_val(val.as_deref());
        }
        b"highlight" => {
            let val = read_val_attribute(tag);
            acc.mark = properties::parse_highlight_val(val.as_deref());
        }
        b"shd" => {
            let val = read_attribute(tag, b"w:val");
            let color = read_attribute(tag, b"w:color");
            let fill = read_attribute(tag, b"w:fill");
            acc.shade = properties::parse_shd(val.as_deref(), color.as_deref(), fill.as_deref());
        }
        b"rFonts" => {
            acc.font = read_rfonts_symbol(tag);
        }
        b"rStyle" => apply_resolved_rpr_style(package, tag, &mut acc.kinds),
        _ => return false,
    }
    true
}

/// Reads the open `<w:rPr>` through its close, resolving run properties.
pub(super) fn parse_rpr(
    input: &mut XmlCursor,
    package: &PackageContext,
) -> Result<ResolvedRunProperties> {
    let mut acc = RunPropertyAccumulator::default();

    loop {
        let event = input.read_owned()?;
        match event {
            quick_xml::events::Event::Start(start) => {
                let local_name = start.local_name();
                let local = local_name.as_ref();
                let _ = apply_resolved_rpr_property(package, local, &start, &mut acc);
                input.skip_subtree(&start)?;
            }
            quick_xml::events::Event::Empty(empty) => {
                let local_name = empty.local_name();
                let local = local_name.as_ref();
                let _ = apply_resolved_rpr_property(package, local, &empty, &mut acc);
            }
            quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"rPr" => {
                return Ok(acc.finish());
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
                return Err(parse_error(
                    "malformed document.xml: unexpected EOF inside <w:rPr>".to_string(),
                ));
            }
        }
    }
}

/// Flattens resolved run properties into the style kinds to emit.
pub(super) fn resolved_run_styles(props: Option<&ResolvedRunProperties>) -> Vec<TextStyleKind> {
    let Some(props) = props else {
        return Vec::new();
    };
    let mut styles = props.kinds.clone();
    if let Some(color) = props.text_color {
        styles.push(TextStyleKind::TextColor(color));
    }
    if let Some(color) = props.mark {
        styles.push(TextStyleKind::Mark(color));
    }
    styles
}

/// Reads the open `<w:numPr>` through its close, resolving `(numId, ilvl)`.
///
/// Returns `None` when no `<w:numId>` is present: a level without a list
/// identifier does not make the paragraph a list item. A missing `<w:ilvl>`
/// defaults to the top level.
pub(super) fn parse_numpr(input: &mut XmlCursor) -> Result<Option<(u32, u32)>> {
    let mut num_id = None;
    let mut ilvl = None;

    loop {
        let event = input.read_owned()?;
        match event {
            quick_xml::events::Event::Empty(empty) => match empty.name().as_ref() {
                b"w:numId" => num_id = parse_u32_attr(&empty, b"w:val"),
                b"w:ilvl" => ilvl = parse_u32_attr(&empty, b"w:val"),
                _ => {}
            },
            quick_xml::events::Event::Start(start) => {
                input.skip_subtree(&start)?;
            }
            quick_xml::events::Event::End(end) if end.name().as_ref() == b"w:numPr" => {
                return Ok(num_id.map(|num_id| (num_id, ilvl.unwrap_or(0))));
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
                return Err(parse_error(
                    "malformed document.xml: unexpected EOF inside <w:numPr>".to_string(),
                ));
            }
        }
    }
}

/// Reads the open `<w:pPr>` through its close, resolving paragraph properties.
///
/// A nested `<w:rPr>` here carries paragraph-mark formatting, which `DocSpec`
/// does not model, so its subtree is dropped.
pub(super) fn parse_ppr(
    input: &mut XmlCursor,
    package: &PackageContext,
) -> Result<ResolvedParagraphProperties> {
    let mut props = ResolvedParagraphProperties::default();

    loop {
        let event = input.read_owned()?;
        match event {
            quick_xml::events::Event::Empty(empty) => match empty.local_name().as_ref() {
                b"jc" => {
                    let val = read_val_attribute(&empty);
                    props.alignment = val.as_deref().and_then(properties::parse_alignment);
                }
                b"pStyle" => {
                    props.classification = read_val_attribute(&empty)
                        .filter(|s| !s.is_empty())
                        .and_then(|s| package.classify_style(&s));
                }
                _ => {}
            },
            quick_xml::events::Event::Start(start) => match start.local_name().as_ref() {
                b"jc" => {
                    let val = read_val_attribute(&start);
                    props.alignment = val.as_deref().and_then(properties::parse_alignment);
                    input.skip_subtree(&start)?;
                }
                b"pStyle" => {
                    props.classification = read_val_attribute(&start)
                        .filter(|s| !s.is_empty())
                        .and_then(|s| package.classify_style(&s));
                    input.skip_subtree(&start)?;
                }
                b"numPr" => {
                    props.list_info = parse_numpr(input)?;
                }
                b"rPr" => {
                    input.skip_subtree(&start)?;
                }
                _ => {}
            },
            quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"pPr" => {
                return Ok(props);
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
                return Err(parse_error(
                    "malformed document.xml: unexpected EOF inside <w:pPr>".to_string(),
                ));
            }
        }
    }
}

/// Reads the open `<w:tcPr>` through its close, resolving cell properties.
pub(super) fn parse_tcpr(input: &mut XmlCursor) -> Result<ResolvedCellProperties> {
    let mut colspan = None;
    loop {
        let event = input.read_owned()?;

        match event {
            quick_xml::events::Event::Empty(empty) => {
                if empty.local_name().as_ref() == b"gridSpan" {
                    let val = read_val_attribute(&empty);
                    colspan = properties::parse_grid_span_value(val.as_deref());
                }
            }
            quick_xml::events::Event::Start(start) => {
                if start.local_name().as_ref() == b"gridSpan" {
                    let val = read_val_attribute(&start);
                    colspan = properties::parse_grid_span_value(val.as_deref());
                }
                input.skip_subtree(&start)?;
            }
            quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"tcPr" => {
                return Ok(ResolvedCellProperties { colspan });
            }
            quick_xml::events::Event::Eof => {
                return Err(parse_error(
                    "malformed document.xml: unexpected EOF inside <w:tcPr>".to_string(),
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

/// Reads the open `<w:trPr>` through its close, resolving row properties.
pub(super) fn parse_trpr(input: &mut XmlCursor) -> Result<ResolvedRowProperties> {
    let mut is_header = false;
    loop {
        let event = input.read_owned()?;

        match event {
            quick_xml::events::Event::Empty(empty) => {
                if empty.local_name().as_ref() == b"tblHeader" {
                    is_header = parse_on_off_attribute(&empty);
                }
            }
            quick_xml::events::Event::Start(start) => {
                if start.local_name().as_ref() == b"tblHeader" {
                    is_header = parse_on_off_attribute(&start);
                }
                input.skip_subtree(&start)?;
            }
            quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"trPr" => {
                return Ok(ResolvedRowProperties { is_header });
            }
            quick_xml::events::Event::Eof => {
                return Err(parse_error(
                    "malformed document.xml: unexpected EOF inside <w:trPr>".to_string(),
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
