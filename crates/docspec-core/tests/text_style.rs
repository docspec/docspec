//! Tests for `TextStyle` builder API.

#[cfg(test)]
mod tests {
    use docspec_core::{Color, TextStyle};

    #[test]
    fn textstyle_default_is_empty() {
        assert_eq!(
            TextStyle::default(),
            TextStyle {
                bold: false,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            }
        );
    }

    #[test]
    fn textstyle_builder_chains() {
        assert_eq!(
            TextStyle::default().bold().italic(),
            TextStyle {
                bold: true,
                italic: true,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            }
        );
    }

    #[test]
    fn textstyle_single_style() {
        assert_eq!(
            TextStyle::default().bold(),
            TextStyle {
                bold: true,
                italic: false,
                code: false,
                strikethrough: false,
                underline: false,
                subscript: false,
                superscript: false,
                mark: None,
            }
        );
    }

    #[test]
    fn textstyle_bold() {
        let style = TextStyle::default().bold();
        assert!(style.bold);
        assert!(!style.italic);
    }

    #[test]
    fn textstyle_italic() {
        let style = TextStyle::default().italic();
        assert!(style.italic);
        assert!(!style.bold);
    }

    #[test]
    fn textstyle_code() {
        let style = TextStyle::default().code();
        assert!(style.code);
    }

    #[test]
    fn textstyle_strikethrough() {
        let style = TextStyle::default().strikethrough();
        assert!(style.strikethrough);
    }

    #[test]
    fn textstyle_underline() {
        let style = TextStyle::default().underline();
        assert!(style.underline);
    }

    #[test]
    fn textstyle_subscript() {
        let style = TextStyle::default().subscript();
        assert!(style.subscript);
    }

    #[test]
    fn textstyle_superscript() {
        let style = TextStyle::default().superscript();
        assert!(style.superscript);
    }

    #[test]
    fn textstyle_mark() {
        let color = Color::Rgb {
            r: 255,
            g: 255,
            b: 0,
        };
        let style = TextStyle::default().mark(color);
        assert_eq!(style.mark, Some(color));
    }

    #[test]
    fn textstyle_all_chained() {
        let color = Color::Rgb { r: 1, g: 2, b: 3 };
        let style = TextStyle::default()
            .bold()
            .italic()
            .code()
            .strikethrough()
            .underline()
            .subscript()
            .superscript()
            .mark(color);
        assert_eq!(
            style,
            TextStyle {
                bold: true,
                italic: true,
                code: true,
                strikethrough: true,
                underline: true,
                subscript: true,
                superscript: true,
                mark: Some(color),
            }
        );
    }
}
