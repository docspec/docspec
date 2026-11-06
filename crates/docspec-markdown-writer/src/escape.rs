//! `CommonMark` text escaping helpers.

use std::io::{self, Write};

use core::iter::Peekable;
use core::str::Chars;

const REPLACEMENT_CHARACTER: &[u8] = "\u{FFFD}".as_bytes();

/// Whitespace dropped at a block start: `CommonMark` strips it from paragraph
/// content anyway, and four spaces or a tab would open an indented code block.
pub(crate) const BLOCK_START_WHITESPACE: [char; 4] = [' ', '\t', '\r', '\n'];

/// Whether `text` could still become an ordered-list marker and needs more
/// text to classify: nothing yet, or 1–9 digits optionally followed by `.`/`)`
/// with the separator not seen yet.
pub(crate) fn block_start_undecided(text: &str) -> bool {
    let rest = text.trim_start_matches(BLOCK_START_WHITESPACE);
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let tail = rest.trim_start_matches(|ch: char| ch.is_ascii_digit());
    matches!((digits, tail), (0, "") | (1..=9, "" | "." | ")"))
}

/// Writes `text` with inline-sensitive `CommonMark` punctuation escaped.
///
/// Normalizes NUL and line endings before escaping and streams output without
/// accumulating the whole escaped text.
pub(crate) fn write_escaped_inline<W>(writer: &mut W, text: &str) -> io::Result<()>
where
    W: Write,
{
    let mut chars = text.chars().peekable();
    while let Some(ch) = next_normalized_char(&mut chars) {
        write_inline_char(writer, ch)?;
    }
    Ok(())
}

/// Writes `text` with inline-sensitive and block-start-sensitive `CommonMark`
/// punctuation escaped.
///
/// Drops leading whitespace, then escapes a first character that would open a
/// heading, block quote, list item, thematic break or fenced code block, and
/// the `.`/`)` of an ordered-list marker. An end of text right after the
/// marker counts as the list separator, so pass the whole block start or hold
/// it while [`block_start_undecided`] is true.
///
/// Normalizes NUL and line endings before escaping and streams output without
/// accumulating the whole escaped text.
pub(crate) fn write_escaped_block_start<W>(writer: &mut W, text: &str) -> io::Result<()>
where
    W: Write,
{
    let mut chars = text
        .trim_start_matches(BLOCK_START_WHITESPACE)
        .chars()
        .peekable();
    let Some(first) = next_normalized_char(&mut chars) else {
        return Ok(());
    };

    match first {
        '#' | '>' | '-' | '+' | '~' => {
            writer.write_all(b"\\")?;
            write_raw_char(writer, first)?;
        }
        '0'..='9' => {
            write_raw_char(writer, first)?;
            write_ordered_list_marker_tail(writer, &mut chars)?;
        }
        ch => write_inline_char(writer, ch)?,
    }

    while let Some(ch) = next_normalized_char(&mut chars) {
        write_inline_char(writer, ch)?;
    }
    Ok(())
}

fn write_ordered_list_marker_tail<W>(
    writer: &mut W,
    chars: &mut Peekable<Chars<'_>>,
) -> io::Result<()>
where
    W: Write,
{
    // CommonMark §5.3: 1–9 digits, then `.` or `)`, then a space, tab or end
    // of line. The first digit is already written; a tenth digit falls through
    // as plain text.
    let mut next = next_normalized_char(chars);
    for _ in 1..9 {
        let Some(digit @ '0'..='9') = next else {
            break;
        };
        write_raw_char(writer, digit)?;
        next = next_normalized_char(chars);
    }
    let Some(marker @ ('.' | ')')) = next else {
        if let Some(ch) = next {
            write_inline_char(writer, ch)?;
        }
        return Ok(());
    };

    let after = next_normalized_char(chars);
    if matches!(after, None | Some(' ' | '\t')) {
        writer.write_all(b"\\")?;
    }
    write_raw_char(writer, marker)?;
    if let Some(ch) = after {
        write_inline_char(writer, ch)?;
    }
    Ok(())
}

fn next_normalized_char(chars: &mut Peekable<Chars<'_>>) -> Option<char> {
    match chars.next()? {
        '\0' => Some('\u{FFFD}'),
        '\r' => {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            Some(' ')
        }
        '\n' => Some(' '),
        ch => Some(ch),
    }
}

fn write_inline_char<W>(writer: &mut W, ch: char) -> io::Result<()>
where
    W: Write,
{
    match ch {
        '\\' => writer.write_all(b"\\\\"),
        '`' => writer.write_all(b"\\`"),
        '*' => writer.write_all(b"\\*"),
        '_' => writer.write_all(b"\\_"),
        '[' => writer.write_all(b"\\["),
        ']' => writer.write_all(b"\\]"),
        '<' => writer.write_all(b"\\<"),
        '&' => writer.write_all(b"\\&"),
        '!' => writer.write_all(b"\\!"),
        // Escaped everywhere so a trailing `#` in a heading never reads as
        // an ATX closing sequence.
        '#' => writer.write_all(b"\\#"),
        '\u{FFFD}' => writer.write_all(REPLACEMENT_CHARACTER),
        ch => write_raw_char(writer, ch),
    }
}

fn write_raw_char<W>(writer: &mut W, ch: char) -> io::Result<()>
where
    W: Write,
{
    let mut buf = [0; 4];
    writer.write_all(ch.encode_utf8(&mut buf).as_bytes())
}

const _: fn(&mut io::Sink, &str) -> io::Result<()> = write_escaped_inline::<io::Sink>;
const _: fn(&mut io::Sink, &str) -> io::Result<()> = write_escaped_block_start::<io::Sink>;

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::{block_start_undecided, write_escaped_block_start, write_escaped_inline};

    fn inline(input: &str) -> Vec<u8> {
        let mut output = Vec::new();
        if let Err(error) = write_escaped_inline(&mut output, input) {
            panic!("inline escape failed: {error}");
        }
        output
    }

    fn block_start(input: &str) -> Vec<u8> {
        let mut output = Vec::new();
        if let Err(error) = write_escaped_block_start(&mut output, input) {
            panic!("block-start escape failed: {error}");
        }
        output
    }

    #[test]
    fn backslash_escaped_first() {
        assert_eq!(inline("\\"), b"\\\\");
    }

    #[test]
    fn backtick_escaped() {
        assert_eq!(inline("`"), b"\\`");
    }

    #[test]
    fn asterisk_escaped() {
        assert_eq!(inline("*"), b"\\*");
    }

    #[test]
    fn underscore_escaped() {
        assert_eq!(inline("_"), b"\\_");
    }

    #[test]
    fn open_bracket_escaped() {
        assert_eq!(inline("["), b"\\[");
    }

    #[test]
    fn close_bracket_escaped() {
        assert_eq!(inline("]"), b"\\]");
    }

    #[test]
    fn less_than_escaped() {
        assert_eq!(inline("<"), b"\\<");
    }

    #[test]
    fn ampersand_escaped() {
        assert_eq!(inline("&"), b"\\&");
    }

    #[test]
    fn exclamation_escaped() {
        assert_eq!(inline("!"), b"\\!");
    }

    #[test]
    fn block_start_hash_escaped() {
        assert_eq!(block_start("# foo"), b"\\# foo");
    }

    #[test]
    fn block_start_greater_than_escaped() {
        assert_eq!(block_start("> foo"), b"\\> foo");
    }

    #[test]
    fn block_start_dash_escaped() {
        assert_eq!(block_start("- foo"), b"\\- foo");
    }

    #[test]
    fn block_start_plus_escaped() {
        assert_eq!(block_start("+ foo"), b"\\+ foo");
    }

    #[test]
    fn block_start_digit_dot_space_escaped() {
        assert_eq!(block_start("1. foo"), b"1\\. foo");
    }

    #[test]
    fn block_start_digit_paren_space_escaped() {
        assert_eq!(block_start("1) foo"), b"1\\) foo");
    }

    #[test]
    fn embedded_newline_normalized() {
        assert_eq!(inline("a\nb"), b"a b");
    }

    #[test]
    fn embedded_crlf_normalized() {
        assert_eq!(inline("a\r\nb"), b"a b");
    }

    #[test]
    fn embedded_cr_normalized() {
        assert_eq!(inline("a\rb"), b"a b");
    }

    #[test]
    fn nul_replaced_with_replacement_character() {
        assert_eq!(inline("\0"), "\u{FFFD}".as_bytes());
    }

    #[test]
    fn unicode_passthrough() {
        assert_eq!(inline("日本語"), "日本語".as_bytes());
    }

    #[test]
    fn empty_string() {
        assert_eq!(inline(""), b"");
    }

    #[test]
    fn mixed_inline_escape_and_newline_normalization() {
        assert_eq!(inline("a*b\nc"), b"a\\*b c");
    }

    #[test]
    fn block_start_digit_dot_nonspace_not_escaped() {
        // "1.x" — marker not followed by space, so NOT an ordered list
        assert_eq!(block_start("1.x"), b"1.x");
    }

    #[test]
    fn block_start_digit_paren_nonspace_not_escaped() {
        // "1)x" — marker not followed by space, so NOT an ordered list
        assert_eq!(block_start("1)x"), b"1)x");
    }

    #[test]
    fn block_start_digit_nonmarker_char_passthrough() {
        // "1a" — digit not followed by . or ), so no escaping needed
        assert_eq!(block_start("1a"), b"1a");
    }

    #[test]
    fn block_start_digit_only_passthrough() {
        // "1" — digit with no following char, no escaping needed
        assert_eq!(block_start("1"), b"1");
    }

    #[test]
    fn block_start_multi_digit_marker_escaped() {
        assert_eq!(block_start("12. foo"), b"12\\. foo");
        assert_eq!(block_start("123456789) foo"), b"123456789\\) foo");
    }

    #[test]
    fn block_start_ten_digit_marker_not_escaped() {
        assert_eq!(block_start("1234567890. foo"), b"1234567890. foo");
    }

    #[test]
    fn block_start_marker_before_tab_or_end_escaped() {
        assert_eq!(block_start("1.\tfoo"), b"1\\.\tfoo");
        assert_eq!(block_start("1."), b"1\\.");
    }

    #[test]
    fn block_start_tilde_escaped() {
        assert_eq!(block_start("~~~ rust"), b"\\~~~ rust");
    }

    #[test]
    fn block_start_leading_whitespace_dropped() {
        assert_eq!(block_start("    code"), b"code");
        assert_eq!(block_start("\t\r\n # foo"), b"\\# foo");
        assert_eq!(block_start("   "), b"");
    }

    #[test]
    fn hash_escaped_inline() {
        assert_eq!(inline("Issue #"), b"Issue \\#");
    }

    #[test]
    fn undecided_while_marker_incomplete() {
        assert!(block_start_undecided(""));
        assert!(block_start_undecided("   "));
        assert!(block_start_undecided("  1"));
        assert!(block_start_undecided("123456789"));
        assert!(block_start_undecided("12."));
        assert!(block_start_undecided("12)"));
    }

    #[test]
    fn decided_once_classifiable() {
        assert!(!block_start_undecided("1234567890"));
        assert!(!block_start_undecided("12. "));
        assert!(!block_start_undecided("12.x"));
        assert!(!block_start_undecided("."));
        assert!(!block_start_undecided("# foo"));
        assert!(!block_start_undecided("a"));
    }
}
