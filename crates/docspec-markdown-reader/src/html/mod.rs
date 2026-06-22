//! HTML-tag translation for raw HTML embedded in markdown.

pub mod tags;
pub mod translator;

use docspec_core::TextStyleKind;

use self::tags::TagIntent;

/// The fill color used for `<mark>` highlights translated from raw HTML.
/// Defaults to browser-conventional yellow (#FFFF00). HTML `style="..."`
/// attributes on `<mark>` are intentionally NOT parsed; the highlight color
/// is always this constant.
pub(crate) const MARK_COLOR: docspec_core::Color = docspec_core::Color::Rgb {
    r: 255,
    g: 255,
    b: 0,
};

/// Translates a [`TagIntent`] into the [`TextStyleKind`] that a
/// [`docspec_core::StyleStack`] should open / close for it.
///
/// Returns `None` for non-style intents (`Heading`, `LineBreak`,
/// `ThematicBreak`, `Ignored`) so the type system enforces that the
/// stack is never asked to track them. Callers in the HTML translator
/// already match on these variants and route them to dedicated handlers;
/// the `None` arm exists to make that contract explicit.
pub(crate) fn tag_intent_to_style_kind(intent: &TagIntent) -> Option<TextStyleKind> {
    Some(match *intent {
        TagIntent::Bold => TextStyleKind::Bold,
        TagIntent::Italic => TextStyleKind::Italic,
        TagIntent::Underline => TextStyleKind::Underline,
        TagIntent::Strikethrough => TextStyleKind::Strikethrough,
        TagIntent::Code => TextStyleKind::Code,
        TagIntent::Subscript => TextStyleKind::Subscript,
        TagIntent::Superscript => TextStyleKind::Superscript,
        TagIntent::Mark => TextStyleKind::Mark(MARK_COLOR),
        TagIntent::LineBreak
        | TagIntent::ThematicBreak
        | TagIntent::Heading(_)
        | TagIntent::Ignored => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // LOAD-BEARING: changing this constant is a behavior-visible change.
    #[test]
    fn mark_color_is_yellow() {
        assert_eq!(
            MARK_COLOR,
            docspec_core::Color::Rgb {
                r: 255,
                g: 255,
                b: 0
            }
        );
    }
}
