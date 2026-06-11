//! Tag-name → semantic intent mapping.
//!
//! Translates lowercase-normalised html5gum tag names (as `&[u8]`) into
//! semantically typed [`TagIntent`] values so the rest of the HTML translation
//! layer can dispatch on kind without repeated byte-slice comparisons.

/// The semantic intent inferred from an HTML tag name.
///
/// html5gum delivers tag names already lowercased, so every match arm in
/// [`tag_intent`] uses lowercase byte-string literals. Unknown or unsupported
/// tags map to [`TagIntent::Ignored`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TagIntent {
    /// Bold text style (`<b>`, `<strong>`).
    Bold,
    /// Italic text style (`<i>`, `<em>`).
    Italic,
    /// Underline text style (`<u>`).
    Underline,
    /// Strikethrough text style (`<s>`, `<strike>`, `<del>`).
    Strikethrough,
    /// Monospace/code text style (`<code>`).
    Code,
    /// Subscript text style (`<sub>`).
    Subscript,
    /// Superscript text style (`<sup>`).
    Superscript,
    /// Highlight/mark text style (`<mark>`).
    Mark,
    /// Hard line break (`<br>`).
    LineBreak,
    /// Thematic break / horizontal rule (`<hr>`).
    ThematicBreak,
    /// Heading block at the given level 1–6 (`<h1>`–`<h6>`).
    ///
    /// The level matches `docspec_core::Event::StartHeading`'s `level: u8` field.
    Heading(u8),
    /// Tag is not in scope — silently ignored by the translator.
    Ignored,
}

/// Maps an html5gum-normalised tag name to its [`TagIntent`].
///
/// The input must be a lowercase byte slice as produced by html5gum's tag-name
/// normalisation. Uppercase or mixed-case input maps to [`TagIntent::Ignored`].
pub(crate) fn tag_intent(name: &[u8]) -> TagIntent {
    match name {
        b"b" | b"strong" => TagIntent::Bold,
        b"i" | b"em" => TagIntent::Italic,
        b"u" => TagIntent::Underline,
        b"s" | b"strike" | b"del" => TagIntent::Strikethrough,
        b"code" => TagIntent::Code,
        b"sub" => TagIntent::Subscript,
        b"sup" => TagIntent::Superscript,
        b"mark" => TagIntent::Mark,
        b"br" => TagIntent::LineBreak,
        b"hr" => TagIntent::ThematicBreak,
        b"h1" => TagIntent::Heading(1),
        b"h2" => TagIntent::Heading(2),
        b"h3" => TagIntent::Heading(3),
        b"h4" => TagIntent::Heading(4),
        b"h5" => TagIntent::Heading(5),
        b"h6" => TagIntent::Heading(6),
        _ => TagIntent::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b_maps_to_bold() {
        assert_eq!(tag_intent(b"b"), TagIntent::Bold);
    }

    #[test]
    fn strong_maps_to_bold() {
        assert_eq!(tag_intent(b"strong"), TagIntent::Bold);
    }

    #[test]
    fn i_maps_to_italic() {
        assert_eq!(tag_intent(b"i"), TagIntent::Italic);
    }

    #[test]
    fn em_maps_to_italic() {
        assert_eq!(tag_intent(b"em"), TagIntent::Italic);
    }

    #[test]
    fn u_maps_to_underline() {
        assert_eq!(tag_intent(b"u"), TagIntent::Underline);
    }

    #[test]
    fn s_maps_to_strikethrough() {
        assert_eq!(tag_intent(b"s"), TagIntent::Strikethrough);
    }

    #[test]
    fn strike_maps_to_strikethrough() {
        assert_eq!(tag_intent(b"strike"), TagIntent::Strikethrough);
    }

    #[test]
    fn del_maps_to_strikethrough() {
        assert_eq!(tag_intent(b"del"), TagIntent::Strikethrough);
    }

    #[test]
    fn code_maps_to_code() {
        assert_eq!(tag_intent(b"code"), TagIntent::Code);
    }

    #[test]
    fn sub_maps_to_subscript() {
        assert_eq!(tag_intent(b"sub"), TagIntent::Subscript);
    }

    #[test]
    fn sup_maps_to_superscript() {
        assert_eq!(tag_intent(b"sup"), TagIntent::Superscript);
    }

    #[test]
    fn mark_maps_to_mark() {
        assert_eq!(tag_intent(b"mark"), TagIntent::Mark);
    }

    #[test]
    fn br_maps_to_line_break() {
        assert_eq!(tag_intent(b"br"), TagIntent::LineBreak);
    }

    #[test]
    fn hr_maps_to_thematic_break() {
        assert_eq!(tag_intent(b"hr"), TagIntent::ThematicBreak);
    }

    #[test]
    fn h1_maps_to_heading_1() {
        assert_eq!(tag_intent(b"h1"), TagIntent::Heading(1));
    }

    #[test]
    fn h2_maps_to_heading_2() {
        assert_eq!(tag_intent(b"h2"), TagIntent::Heading(2));
    }

    #[test]
    fn h3_maps_to_heading_3() {
        assert_eq!(tag_intent(b"h3"), TagIntent::Heading(3));
    }

    #[test]
    fn h4_maps_to_heading_4() {
        assert_eq!(tag_intent(b"h4"), TagIntent::Heading(4));
    }

    #[test]
    fn h5_maps_to_heading_5() {
        assert_eq!(tag_intent(b"h5"), TagIntent::Heading(5));
    }

    #[test]
    fn h6_maps_to_heading_6() {
        assert_eq!(tag_intent(b"h6"), TagIntent::Heading(6));
    }

    #[test]
    fn div_maps_to_ignored() {
        assert_eq!(tag_intent(b"div"), TagIntent::Ignored);
    }

    #[test]
    fn span_maps_to_ignored() {
        assert_eq!(tag_intent(b"span"), TagIntent::Ignored);
    }

    #[test]
    fn p_maps_to_ignored() {
        assert_eq!(tag_intent(b"p"), TagIntent::Ignored);
    }

    #[test]
    fn a_maps_to_ignored() {
        assert_eq!(tag_intent(b"a"), TagIntent::Ignored);
    }

    #[test]
    fn img_maps_to_ignored() {
        assert_eq!(tag_intent(b"img"), TagIntent::Ignored);
    }

    #[test]
    fn empty_maps_to_ignored() {
        assert_eq!(tag_intent(b""), TagIntent::Ignored);
    }

    #[test]
    fn uppercase_h1_maps_to_ignored() {
        // html5gum normalises to lowercase upstream; uppercase confirms input contract
        assert_eq!(tag_intent(b"H1"), TagIntent::Ignored);
    }
}
