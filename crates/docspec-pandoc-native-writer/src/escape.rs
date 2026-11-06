//! Haskell string escape utilities for Pandoc native output.

use std::io::{self, Write};

/// An escape after which the next character may need a `\&` gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gap {
    /// `\SO` was written: a following `H` would read back as `\SOH` (U+0001).
    ShiftOut,
    /// A decimal escape `\NNN` was written: a following digit would extend it.
    Decimal,
}

impl Gap {
    /// Whether `next` must be preceded by `\&` to end this escape.
    #[inline]
    const fn is_needed_before(self, next: char) -> bool {
        match self {
            Self::ShiftOut => next == 'H',
            Self::Decimal => next.is_ascii_digit(),
        }
    }
}

/// A Haskell string literal written in pieces, so the content never has to be
/// held in memory as a whole.
///
/// [`open`](Self::open) writes the opening `"`, [`write`](Self::write) escapes
/// one chunk of content, and [`close`](Self::close) writes the closing `"`.
/// A gap escape that depends on the next character is resolved even when that
/// character arrives in a later chunk. The escape table is the one documented
/// on [`write_haskell_string`].
#[derive(Debug)]
pub struct HaskellString {
    /// The gap the last escape may need, decided by the next character written.
    pending_gap: Option<Gap>,
}

impl HaskellString {
    /// Writes the opening `"` and returns the open literal.
    #[inline]
    pub fn open(out: &mut dyn Write) -> io::Result<Self> {
        out.write_all(b"\"")?;
        Ok(Self { pending_gap: None })
    }

    /// Escapes `chunk` into the open literal.
    #[inline]
    pub fn write(&mut self, out: &mut dyn Write, chunk: &str) -> io::Result<()> {
        for ch in chunk.chars() {
            if let Some(gap) = self.pending_gap.take() {
                if gap.is_needed_before(ch) {
                    out.write_all(b"\\&")?;
                }
            }
            self.pending_gap = write_escaped_char(out, ch)?;
        }
        Ok(())
    }

    /// Writes the closing `"` and consumes the literal, so nothing can be
    /// written into it afterwards.
    #[inline]
    pub fn close(self, out: &mut dyn Write) -> io::Result<()> {
        // `"` ends any escape, so a pending gap is dropped without a `\&`.
        let Self { .. } = self;
        out.write_all(b"\"")
    }
}

/// Writes `ch` escaped, and returns the gap the next character may need.
fn write_escaped_char(out: &mut dyn Write, ch: char) -> io::Result<Option<Gap>> {
    match ch {
        '"' => out.write_all(b"\\\"")?,
        '\\' => out.write_all(b"\\\\")?,
        // No gap escape needed after \NUL (not a decimal escape)
        '\x00' => out.write_all(b"\\NUL")?,
        '\x07' => out.write_all(b"\\a")?,
        '\x08' => out.write_all(b"\\b")?,
        '\x09' => out.write_all(b"\\t")?,
        '\x0A' => out.write_all(b"\\n")?,
        '\x0B' => out.write_all(b"\\v")?,
        '\x0C' => out.write_all(b"\\f")?,
        '\x0D' => out.write_all(b"\\r")?,
        '\x0E' => {
            out.write_all(b"\\SO")?;
            return Ok(Some(Gap::ShiftOut));
        }
        '\x0F' => out.write_all(b"\\SI")?,
        '\x7F' => out.write_all(b"\\DEL")?,
        c if u32::from(c) < 0x20 => {
            // Other control chars 0x01-0x06, 0x10-0x1F: decimal escape
            write!(out, "\\{}", u32::from(c))?;
            return Ok(Some(Gap::Decimal));
        }
        c => {
            // All other chars including non-ASCII: emit raw UTF-8
            let mut buf: [u8; 4] = [0; 4];
            out.write_all(c.encode_utf8(&mut buf).as_bytes())?;
        }
    }
    Ok(None)
}

/// Writes a Haskell `Read`-safe string literal to `out`.
///
/// Emits the opening `"`, the escaped body, and the closing `"`.
/// Streams character-by-character; never accumulates the whole output in a
/// `String` or `Vec<u8>`. Use [`HaskellString`] when the content arrives in
/// several pieces.
///
/// # Escape table
///
/// | Input | Output |
/// |-------|--------|
/// | `"` (0x22) | `\"` |
/// | `\` (0x5C) | `\\` |
/// | 0x00 | `\NUL` |
/// | 0x07 | `\a` |
/// | 0x08 | `\b` |
/// | 0x09 | `\t` |
/// | 0x0A | `\n` |
/// | 0x0B | `\v` |
/// | 0x0C | `\f` |
/// | 0x0D | `\r` |
/// | 0x0E | `\SO` |
/// | 0x0F | `\SI` |
/// | 0x7F | `\DEL` |
/// | 0x01–0x06, 0x10–0x1F | `\NNN` decimal |
/// | ≥ 0x80 | raw UTF-8 bytes |
///
/// Gap escapes: `\SO` followed by `H` emits `\SO\&H`; `\NNN` followed by
/// a digit `0`–`9` emits `\NNN\&<digit>`.
pub fn write_haskell_string(out: &mut dyn Write, content: &str) -> io::Result<()> {
    let mut literal = HaskellString::open(out)?;
    literal.write(out, content)?;
    literal.close(out)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::{write_haskell_string, HaskellString};

    fn escape(s: &str) -> String {
        let mut buf = Vec::new();
        write_haskell_string(&mut buf, s).unwrap();
        String::from_utf8(buf).unwrap()
    }

    /// Escapes `chunks` as one literal, one `write` call per chunk.
    fn escape_chunks(chunks: &[&str]) -> String {
        let mut buf = Vec::new();
        let mut literal = HaskellString::open(&mut buf).unwrap();
        for chunk in chunks {
            literal.write(&mut buf, chunk).unwrap();
        }
        literal.close(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn chunks_without_content_emit_empty_literal() {
        assert_eq!(escape_chunks(&[]), "\"\"");
    }

    #[test]
    fn chunks_concatenate_into_one_literal() {
        assert_eq!(escape_chunks(&["a\"", "b\\", "\n"]), "\"a\\\"b\\\\\\n\"");
    }

    #[test]
    fn gap_escape_so_carries_across_chunks() {
        assert_eq!(escape_chunks(&["\x0E", "H"]), "\"\\SO\\&H\"");
    }

    #[test]
    fn gap_escape_so_carries_across_empty_chunk() {
        assert_eq!(escape_chunks(&["\x0E", "", "H"]), "\"\\SO\\&H\"");
    }

    #[test]
    fn no_gap_escape_for_so_then_lowercase_h_across_chunks() {
        assert_eq!(escape_chunks(&["\x0E", "h"]), "\"\\SOh\"");
    }

    #[test]
    fn gap_escape_decimal_carries_across_chunks() {
        assert_eq!(escape_chunks(&["\x16", "9"]), "\"\\22\\&9\"");
    }

    #[test]
    fn no_gap_escape_for_decimal_then_non_digit_across_chunks() {
        assert_eq!(escape_chunks(&["\x01", "a"]), "\"\\1a\"");
    }

    #[test]
    fn no_gap_escape_before_closing_quote() {
        assert_eq!(escape_chunks(&["\x0E"]), "\"\\SO\"");
        assert_eq!(escape_chunks(&["\x01"]), "\"\\1\"");
    }

    #[test]
    fn gap_resolves_against_next_char_only() {
        // The newline ends the \SO escape; the H after it needs no gap.
        assert_eq!(escape_chunks(&["\x0E", "\nH"]), "\"\\SO\\nH\"");
    }

    #[test]
    fn empty_string() {
        assert_eq!(escape(""), "\"\"");
    }

    #[test]
    fn plain_ascii() {
        assert_eq!(escape("hi"), "\"hi\"");
    }

    #[test]
    fn escape_double_quote() {
        assert_eq!(escape("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn escape_backslash() {
        assert_eq!(escape("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn escape_nul() {
        assert_eq!(escape("\x00"), "\"\\NUL\"");
    }

    #[test]
    fn escape_bel() {
        assert_eq!(escape("\x07"), "\"\\a\"");
    }

    #[test]
    fn escape_bs() {
        assert_eq!(escape("\x08"), "\"\\b\"");
    }

    #[test]
    fn escape_ht() {
        assert_eq!(escape("\x09"), "\"\\t\"");
    }

    #[test]
    fn escape_lf() {
        assert_eq!(escape("\x0A"), "\"\\n\"");
    }

    #[test]
    fn escape_vt() {
        assert_eq!(escape("\x0B"), "\"\\v\"");
    }

    #[test]
    fn escape_ff() {
        assert_eq!(escape("\x0C"), "\"\\f\"");
    }

    #[test]
    fn escape_cr() {
        assert_eq!(escape("\x0D"), "\"\\r\"");
    }

    #[test]
    fn escape_so() {
        assert_eq!(escape("\x0E"), "\"\\SO\"");
    }

    #[test]
    fn escape_si() {
        assert_eq!(escape("\x0F"), "\"\\SI\"");
    }

    #[test]
    fn escape_del() {
        assert_eq!(escape("\x7F"), "\"\\DEL\"");
    }

    #[test]
    fn escape_ctrl_0x01() {
        assert_eq!(escape("\x01"), "\"\\1\"");
    }

    #[test]
    fn escape_ctrl_0x16() {
        assert_eq!(escape("\x16"), "\"\\22\"");
    }

    #[test]
    fn escape_ctrl_0x1f() {
        assert_eq!(escape("\x1F"), "\"\\31\"");
    }

    #[test]
    fn unicode_right_single_quote_raw_utf8() {
        // U+2019 = 0xE2 0x80 0x99 in UTF-8
        let result = escape("\u{2019}");
        assert_eq!(result.as_bytes(), b"\"\xe2\x80\x99\"");
    }

    #[test]
    fn gap_escape_so_followed_by_uppercase_h() {
        // \x0E followed by 'H' -> "\SO\&H"
        assert_eq!(escape("\x0EH"), "\"\\SO\\&H\"");
    }

    #[test]
    fn gap_escape_so_followed_by_lowercase_h() {
        // \x0E followed by 'h' -> "\SOh" (no gap escape; 'h' is not 'H')
        assert_eq!(escape("\x0Eh"), "\"\\SOh\"");
    }

    #[test]
    fn gap_escape_decimal_followed_by_digit() {
        // 0x01 followed by '5' -> "\1\&5"
        assert_eq!(escape("\x015"), "\"\\1\\&5\"");
    }

    #[test]
    fn gap_escape_decimal_followed_by_non_digit() {
        // 0x01 followed by 'a' -> "\1a" (no gap escape)
        assert_eq!(escape("\x01a"), "\"\\1a\"");
    }

    #[test]
    fn gap_escape_decimal_0x16_followed_by_digit() {
        // 0x16 followed by '9' -> "\22\&9"
        assert_eq!(escape("\x169"), "\"\\22\\&9\"");
    }

    #[test]
    fn no_gap_escape_for_non_ascii_followed_by_digit() {
        // U+00A0 followed by '5' -> raw UTF-8 bytes 0xC2 0xA0 then '5'
        // No gap escape because non-ASCII emits raw bytes, not a numeric escape
        let result = escape("\u{00A0}5");
        assert_eq!(result.as_bytes(), b"\"\xc2\xa05\"");
    }

    #[test]
    fn mixed_complex_string() {
        // "hello\n\"world\"\\done\x01\x0EHellow"
        let input = "hello\n\"world\"\\done\x01\x0EHellow";
        let result = escape(input);
        assert_eq!(result, "\"hello\\n\\\"world\\\"\\\\done\\1\\SO\\&Hellow\"");
    }
}
