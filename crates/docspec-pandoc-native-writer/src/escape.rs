//! Haskell string escape utilities for Pandoc native output.

/// Writes a Haskell `Read`-safe string literal to `out`.
///
/// Emits the opening `"`, the escaped body, and the closing `"`.
/// Streams character-by-character; never accumulates the whole output in a
/// `String` or `Vec<u8>`.
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
pub fn write_haskell_string(out: &mut dyn std::io::Write, content: &str) -> std::io::Result<()> {
    out.write_all(b"\"")?;
    let mut chars = content.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => out.write_all(b"\\\"")?,
            '\\' => out.write_all(b"\\\\")?,
            '\x00' => {
                out.write_all(b"\\NUL")?;
                // No gap escape needed after \NUL (not a decimal escape)
            }
            '\x07' => out.write_all(b"\\a")?,
            '\x08' => out.write_all(b"\\b")?,
            '\x09' => out.write_all(b"\\t")?,
            '\x0A' => out.write_all(b"\\n")?,
            '\x0B' => out.write_all(b"\\v")?,
            '\x0C' => out.write_all(b"\\f")?,
            '\x0D' => out.write_all(b"\\r")?,
            '\x0E' => {
                out.write_all(b"\\SO")?;
                // Gap escape: \SO followed by H would be read as \SOH (U+0001)
                if chars.peek() == Some(&'H') {
                    out.write_all(b"\\&")?;
                }
            }
            '\x0F' => out.write_all(b"\\SI")?,
            '\x7F' => out.write_all(b"\\DEL")?,
            c if u32::from(c) < 0x20 => {
                // Other control chars 0x01-0x06, 0x10-0x1F: decimal escape
                let n = u32::from(c);
                write!(out, "\\{n}")?;
                // Gap escape: decimal escape followed by digit would extend the number
                if matches!(chars.peek(), Some('0'..='9')) {
                    out.write_all(b"\\&")?;
                }
            }
            c => {
                // All other chars including non-ASCII: emit raw UTF-8
                let mut buf: [u8; 4] = [0; 4];
                out.write_all(c.encode_utf8(&mut buf).as_bytes())?;
            }
        }
    }
    out.write_all(b"\"")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::write_haskell_string;

    fn escape(s: &str) -> String {
        let mut buf = Vec::new();
        write_haskell_string(&mut buf, s).unwrap();
        String::from_utf8(buf).unwrap()
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
