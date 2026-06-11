//! HTML-tag translation for raw HTML embedded in markdown.

pub mod stack;
pub mod tags;
pub mod translator;

/// The fill color used for `<mark>` highlights translated from raw HTML.
/// Defaults to browser-conventional yellow (#FFFF00). HTML `style="..."`
/// attributes on `<mark>` are intentionally NOT parsed; the highlight color
/// is always this constant.
pub(crate) const MARK_COLOR: docspec_core::Color = docspec_core::Color::Rgb {
    r: 255,
    g: 255,
    b: 0,
};

/// Maximum number of simultaneously-open HTML inline-style spans.
/// Beyond this depth, additional opens are silently ignored to bound
/// memory under adversarial or malformed input.
pub(crate) const MAX_HTML_STYLE_DEPTH: usize = 32;

#[cfg(test)]
mod tests {
    use super::*;

    // LOAD-BEARING: changing these constants is a behavior-visible change.
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

    #[test]
    fn max_html_style_depth_is_32() {
        assert_eq!(MAX_HTML_STYLE_DEPTH, 32);
    }
}
