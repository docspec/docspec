//! HYPERLINK complex-field instruction parsing.
//!
//! Legacy DOCX (pre-Word 2007, and content emitted by some toolchains such as
//! Pandoc) encodes hyperlinks using the `<w:fldChar>` + `<w:instrText>` complex
//! field syntax from OOXML §17.16.5.25 instead of the modern `<w:hyperlink>`
//! element. The on-the-wire form looks like:
//!
//! ```xml
//! <w:r><w:fldChar w:fldCharType="begin"/></w:r>
//! <w:r><w:instrText> HYPERLINK "https://example.com" \o "tooltip" </w:instrText></w:r>
//! <w:r><w:fldChar w:fldCharType="separate"/></w:r>
//! <w:r><w:t>Displayed text</w:t></w:r>
//! <w:r><w:fldChar w:fldCharType="end"/></w:r>
//! ```
//!
//! This module parses the instruction string carried by `<w:instrText>` into
//! the URL, anchor, and tooltip the field describes. The DOCX reader uses the
//! result to wrap the buffered display content in `StartLink` / `EndLink`
//! events, matching the semantics of the modern `<w:hyperlink>` element.
//!
//! Non-`HYPERLINK` instructions (`PAGE`, `REF`, `TOC`, `ADDIN ...`, etc.) yield
//! `None` from [`parse_hyperlink_instruction`] so the reader can pass their
//! display content through unwrapped.

/// Parsed arguments from a `HYPERLINK` field instruction.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct HyperlinkFieldArgs {
    /// External URL — the first positional argument after `HYPERLINK`.
    pub url: Option<String>,
    /// Bookmark / fragment anchor — the argument to `\l "..."`.
    pub anchor: Option<String>,
    /// Tooltip text — the argument to `\o "..."`.
    pub tooltip: Option<String>,
}

/// Parses a field instruction string and returns `Some` if it is a well-formed
/// `HYPERLINK` field, `None` otherwise.
///
/// Recognized switches (OOXML §17.16.5.25, p. 1216):
///
/// - `\l "anchor"` — bookmark / fragment anchor.
/// - `\o "tooltip"` — `ScreenTip` text.
/// - `\t "frame"` — target frame name (parsed and discarded; the event model
///   has no equivalent).
/// - `\m`, `\n` — image-map / new-window flags (parsed and discarded).
///
/// Unknown switches are skipped. Extra positional arguments after the URL are
/// ignored. The command name match is ASCII case-insensitive (matching the
/// historical Word behavior).
pub(crate) fn parse_hyperlink_instruction(instr: &str) -> Option<HyperlinkFieldArgs> {
    let tokens = tokenize_field_instruction(instr);
    let mut iter = tokens.into_iter();
    let cmd = iter.next()?;
    if !cmd.eq_ignore_ascii_case("HYPERLINK") {
        return None;
    }

    let mut args = HyperlinkFieldArgs::default();
    while let Some(tok) = iter.next() {
        if let Some(switch) = tok.strip_prefix('\\') {
            match switch {
                "l" => args.anchor = iter.next(),
                "o" => args.tooltip = iter.next(),
                "t" => {
                    // Consume the frame argument; we have no event-model equivalent.
                    let _ = iter.next();
                }
                _ => {
                    // `\m`, `\n`, and any unknown switch — no argument to consume.
                }
            }
        } else if args.url.is_none() {
            args.url = Some(tok);
        } else {
            // Extra positional tokens after the URL are silently ignored.
        }
    }
    Some(args)
}

/// Resolves the effective `href` for a parsed `HYPERLINK` field, matching the
/// existing `<w:hyperlink>` reader semantics:
///
/// - When a URL is present, the anchor is dropped (URL alone is the href).
/// - When only an anchor is present, the href is `#anchor`.
/// - An empty anchor with no URL yields `None`.
pub(crate) fn resolve_field_href(args: &HyperlinkFieldArgs) -> Option<String> {
    args.url.clone().or_else(|| {
        args.anchor
            .as_ref()
            .filter(|a| !a.is_empty())
            .map(|anchor| format!("#{anchor}"))
    })
}

/// Splits a field instruction into tokens.
///
/// Tokenization rules:
/// - Whitespace separates tokens.
/// - A `"..."` span is a single token whose surrounding double-quotes are
///   stripped; whitespace inside quotes is preserved.
/// - A switch marker (`\l`, `\o`, ...) is a single token starting with `\`.
/// - An unterminated quoted span flushes its accumulated content as the final
///   token (graceful recovery for malformed instructions).
fn tokenize_field_instruction(instr: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in instr.chars() {
        if in_quotes {
            if c == '"' {
                in_quotes = false;
                tokens.push(core::mem::take(&mut current));
            } else {
                current.push(c);
            }
        } else if c == '"' {
            if !current.is_empty() {
                tokens.push(core::mem::take(&mut current));
            }
            in_quotes = true;
        } else if c.is_whitespace() {
            if !current.is_empty() {
                tokens.push(core::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn tokenizer_splits_on_whitespace_and_quotes() {
        let tokens = tokenize_field_instruction(r#" HYPERLINK "hello world" \l "in anchor" "#);
        assert_eq!(
            tokens,
            vec![
                "HYPERLINK".to_string(),
                "hello world".to_string(),
                "\\l".to_string(),
                "in anchor".to_string(),
            ]
        );
    }

    #[test]
    fn tokenizer_handles_adjacent_switch_and_quote() {
        let tokens = tokenize_field_instruction(r#"HYPERLINK "url"\l"anchor""#);
        assert_eq!(
            tokens,
            vec![
                "HYPERLINK".to_string(),
                "url".to_string(),
                "\\l".to_string(),
                "anchor".to_string(),
            ]
        );
    }

    #[test]
    fn tokenizer_recovers_from_unterminated_quote() {
        let tokens = tokenize_field_instruction(r#" HYPERLINK "missing-close "#);
        assert_eq!(
            tokens,
            vec!["HYPERLINK".to_string(), "missing-close ".to_string()]
        );
    }

    #[test]
    fn tokenizer_returns_empty_for_only_whitespace() {
        assert_eq!(tokenize_field_instruction("    "), Vec::<String>::new());
        assert_eq!(tokenize_field_instruction(""), Vec::<String>::new());
    }

    #[test]
    fn tokenizer_preserves_empty_quoted_token() {
        let tokens = tokenize_field_instruction(r#" HYPERLINK "" \l "x" "#);
        assert_eq!(
            tokens,
            vec![
                "HYPERLINK".to_string(),
                String::new(),
                "\\l".to_string(),
                "x".to_string(),
            ]
        );
    }

    #[test]
    fn parses_hyperlink_with_only_url() {
        let args = parse_hyperlink_instruction(r#" HYPERLINK "https://example.com" "#).unwrap();
        assert_eq!(args.url.as_deref(), Some("https://example.com"));
        assert_eq!(args.anchor, None);
        assert_eq!(args.tooltip, None);
    }

    #[test]
    fn parses_hyperlink_with_url_and_anchor() {
        let args =
            parse_hyperlink_instruction(r#" HYPERLINK "https://example.com/page" \l "section-1" "#)
                .unwrap();
        assert_eq!(args.url.as_deref(), Some("https://example.com/page"));
        assert_eq!(args.anchor.as_deref(), Some("section-1"));
    }

    #[test]
    fn parses_hyperlink_with_url_and_tooltip() {
        let args = parse_hyperlink_instruction(
            r#" HYPERLINK "https://example.com" \o "Click here for more" "#,
        )
        .unwrap();
        assert_eq!(args.url.as_deref(), Some("https://example.com"));
        assert_eq!(args.tooltip.as_deref(), Some("Click here for more"));
    }

    #[test]
    fn parses_hyperlink_with_anchor_only() {
        let args = parse_hyperlink_instruction(r#" HYPERLINK \l "bookmark-id" "#).unwrap();
        assert_eq!(args.url, None);
        assert_eq!(args.anchor.as_deref(), Some("bookmark-id"));
    }

    #[test]
    fn parses_hyperlink_with_all_switches() {
        let args = parse_hyperlink_instruction(
            r#" HYPERLINK "https://example.com" \t "_blank" \o "tip" \l "frag" \m \n "#,
        )
        .unwrap();
        assert_eq!(args.url.as_deref(), Some("https://example.com"));
        assert_eq!(args.anchor.as_deref(), Some("frag"));
        assert_eq!(args.tooltip.as_deref(), Some("tip"));
    }

    #[test]
    fn command_name_is_case_insensitive() {
        let lower = parse_hyperlink_instruction(r#" hyperlink "https://example.com" "#).unwrap();
        assert_eq!(lower.url.as_deref(), Some("https://example.com"));
        let mixed = parse_hyperlink_instruction(r#" HyperLink "https://example.com" "#).unwrap();
        assert_eq!(mixed.url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn rejects_non_hyperlink_commands() {
        assert!(parse_hyperlink_instruction(" PAGE ").is_none());
        assert!(parse_hyperlink_instruction(" REF _Ref71265628 \\h ").is_none());
        assert!(parse_hyperlink_instruction(" PAGEREF _Toc74303302 \\h ").is_none());
        assert!(parse_hyperlink_instruction(" TOC \\o \"1-3\" \\u ").is_none());
        assert!(parse_hyperlink_instruction(" SEQ Table \\* ARABIC ").is_none());
        assert!(parse_hyperlink_instruction(" XE \"French\" ").is_none());
        assert!(parse_hyperlink_instruction(" INDEX  \\* MERGEFORMAT ").is_none());
        assert!(parse_hyperlink_instruction(" CREATEDATE  \\* MERGEFORMAT ").is_none());
        assert!(
            parse_hyperlink_instruction(r#"ADDIN CSL_CITATION {"citationItems":[]}"#).is_none()
        );
        assert!(
            parse_hyperlink_instruction(" ADDIN ZOTERO_ITEM CSL_CITATION {\"x\":1} ").is_none()
        );
    }

    #[test]
    fn rejects_empty_instruction() {
        assert!(parse_hyperlink_instruction("").is_none());
        assert!(parse_hyperlink_instruction("   ").is_none());
        assert!(parse_hyperlink_instruction("\t\n").is_none());
    }

    #[test]
    fn unknown_switches_are_skipped_without_consuming_following_arg() {
        // `\x` is unknown — it should not swallow the following positional `url`.
        let args = parse_hyperlink_instruction(r#" HYPERLINK \x "https://example.com" "#).unwrap();
        assert_eq!(args.url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn extra_positional_after_url_is_ignored() {
        let args = parse_hyperlink_instruction(
            r#" HYPERLINK "https://a.example" "https://b.example" \l "anchor" "#,
        )
        .unwrap();
        assert_eq!(args.url.as_deref(), Some("https://a.example"));
        assert_eq!(args.anchor.as_deref(), Some("anchor"));
    }

    #[test]
    fn resolve_href_prefers_url_when_both_present() {
        let args = parse_hyperlink_instruction(r#" HYPERLINK "https://example.com" \l "ignored" "#)
            .unwrap();
        assert_eq!(
            resolve_field_href(&args).as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn resolve_href_uses_anchor_when_url_absent() {
        let args = parse_hyperlink_instruction(r#" HYPERLINK \l "bookmark" "#).unwrap();
        assert_eq!(resolve_field_href(&args).as_deref(), Some("#bookmark"));
    }

    #[test]
    fn resolve_href_returns_none_for_empty_anchor() {
        let args = HyperlinkFieldArgs {
            url: None,
            anchor: Some(String::new()),
            tooltip: None,
        };
        assert_eq!(resolve_field_href(&args), None);
    }

    #[test]
    fn resolve_href_returns_none_when_url_and_anchor_absent() {
        let args = HyperlinkFieldArgs::default();
        assert_eq!(resolve_field_href(&args), None);
    }

    #[test]
    fn parses_real_world_pandoc_instruction_with_decoded_ampersands() {
        // The XML payload is `&amp;`-escaped in the instrText, but quick-xml
        // decodes it before handing it to us, so the parser sees real `&`.
        let raw = r#" HYPERLINK "https://books.google.com/books?id=sp_Zcb9ot90C&lpg=PR4&hl=zh-CN&pg=PA19" \l "v=onepage&q&f=true" "#;
        let args = parse_hyperlink_instruction(raw).unwrap();
        assert_eq!(
            args.url.as_deref(),
            Some("https://books.google.com/books?id=sp_Zcb9ot90C&lpg=PR4&hl=zh-CN&pg=PA19")
        );
        assert_eq!(args.anchor.as_deref(), Some("v=onepage&q&f=true"));
        assert_eq!(args.tooltip, None);
        // When URL is present, anchor is dropped — matches existing `<w:hyperlink>` behavior.
        assert_eq!(
            resolve_field_href(&args).as_deref(),
            Some("https://books.google.com/books?id=sp_Zcb9ot90C&lpg=PR4&hl=zh-CN&pg=PA19")
        );
    }
}
