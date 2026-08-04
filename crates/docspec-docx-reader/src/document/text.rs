//! Text extraction and symbol-font normalization for run content.
//!
//! These are leaf operations over a bounded region of the document: `<w:t>`
//! text, an entity reference, or a single `<w:sym>` element. None of them
//! recurse into document structure, so they stay independent of the parser's
//! descent and take only the cursor and the resolved font they actually need.

use docspec_core::Result;
use quick_xml::events::{BytesRef, BytesStart};

use super::input::XmlCursor;
use super::{parse_error, read_attribute};
use crate::symbol_fonts::SymbolFont;

/// Reads the character data of the currently open `<w:t>` element.
///
/// Consumes the cursor through the matching `</w:t>`, concatenating text,
/// CDATA, and resolved entity references. Nested elements are skipped whole.
/// An early EOF yields the text gathered so far rather than an error, matching
/// the reader's tolerance for truncated runs.
pub(super) fn collect_text_content(input: &mut XmlCursor) -> Result<String> {
    let mut content = String::new();
    loop {
        let event = input.read_owned()?;
        match event {
            quick_xml::events::Event::Text(text) => {
                let decoded = text
                    .decode()
                    .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
                let unescaped = quick_xml::escape::unescape(&decoded)
                    .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
                content.push_str(&unescaped);
            }
            quick_xml::events::Event::CData(cdata) => {
                let bytes = cdata.into_inner();
                let text = core::str::from_utf8(&bytes)
                    .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
                content.push_str(text);
            }
            quick_xml::events::Event::GeneralRef(reference) => {
                content.push_str(&decode_general_ref(&reference)?);
            }
            quick_xml::events::Event::End(end) if end.local_name().as_ref() == b"t" => {
                return Ok(content);
            }
            quick_xml::events::Event::Start(start) => {
                input.skip_subtree(&start)?;
            }
            quick_xml::events::Event::Empty(_)
            | quick_xml::events::Event::End(_)
            | quick_xml::events::Event::Comment(_)
            | quick_xml::events::Event::Decl(_)
            | quick_xml::events::Event::PI(_)
            | quick_xml::events::Event::DocType(_) => {}
            quick_xml::events::Event::Eof => return Ok(content),
        }
    }
}

/// Remaps text authored in a symbol font to its Unicode equivalents.
///
/// Word stores symbol-font glyphs either in the Private Use Area (`U+F020`
/// upward) or as raw Latin-1 codepoints. Both forms index the same table, so
/// each is reduced to a `u8` key before lookup. Codepoints the font does not
/// map are dropped, and text in a non-symbol font is returned untouched.
pub(super) fn normalize_symbol_text(font: Option<SymbolFont>, text: String) -> String {
    let Some(font) = font else {
        return text;
    };

    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
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
    out
}

/// Resolves an XML entity reference to the text it stands for.
///
/// `quick-xml` surfaces the reference name without its delimiters, so they are
/// restored before unescaping to reuse the standard entity table.
pub(super) fn decode_general_ref(reference: &BytesRef<'_>) -> Result<String> {
    let decoded = reference
        .decode()
        .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
    let escaped = format!("&{decoded};");
    let unescaped = quick_xml::escape::unescape(&escaped)
        .map_err(|err| parse_error(format!("malformed document.xml: {err}")))?;
    Ok(unescaped.into_owned())
}

/// Resolves a `<w:sym>` element to its Unicode character.
///
/// The element's own `w:font` takes precedence over `run_font`, the font
/// resolved from the enclosing run's properties. Returns `None` when the
/// codepoint is malformed, no symbol font applies, or the font does not map it.
pub(super) fn resolve_sym_char(
    run_font: Option<SymbolFont>,
    tag: &BytesStart<'_>,
) -> Option<String> {
    let char_hex = read_attribute(tag, b"w:char")?;
    let key = crate::properties::parse_sym_char(&char_hex)?;
    let font = read_attribute(tag, b"w:font")
        .and_then(|name| SymbolFont::from_name(&name))
        .or(run_font)?;
    font.convert(key).map(String::from)
}
