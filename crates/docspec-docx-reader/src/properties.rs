//! OOXML value parsers for run and paragraph properties.

use docspec_core::{Color, TextAlignment};

/// Parses an `ST_OnOff` value (ECMA-376 §22.9.2.7).
///
/// Element-present with no `w:val` = ON. Unknown values treated as ON (lenient).
pub fn parse_on_off(val: Option<&str>) -> bool {
    !matches!(val, Some("false" | "0" | "off"))
}

/// Vertical alignment for text (subscript, superscript, or baseline).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertAlign {
    /// Baseline (default, no vertical shift).
    None,
    /// Subscript (below baseline).
    Subscript,
    /// Superscript (above baseline).
    Superscript,
}

/// Parses a `ST_VerticalAlignRun` value. `baseline` and unknown values map to `None`.
pub fn parse_vert_align(val: Option<&str>) -> VertAlign {
    match val {
        Some("subscript") => VertAlign::Subscript,
        Some("superscript") => VertAlign::Superscript,
        _ => VertAlign::None,
    }
}

/// Parses `w:u` underline state. Unlike `ST_OnOff`, element-present with no `w:val` = OFF.
/// Only a non-`"none"` `w:val` enables underline (ECMA-376 §17.3.2.40).
pub fn parse_underline_on(val: Option<&str>) -> bool {
    match val {
        None | Some("none") => false,
        Some(_) => true,
    }
}

/// Parses `w:jc` paragraph alignment. `start`→Left, `end`→Right (no `BiDi` tracking).
pub fn parse_alignment(val: &str) -> Option<TextAlignment> {
    match val {
        "left" | "start" => Some(TextAlignment::Left),
        "right" | "end" => Some(TextAlignment::Right),
        "center" => Some(TextAlignment::Center),
        "both" | "distribute" => Some(TextAlignment::Justify),
        _ => None,
    }
}

/// Parses a hex color string (with or without leading `#`, 3 or 6 hex digits) into a `Color`.
///
/// Returns `None` for `"auto"`, empty strings, non-hex input, or lengths other than 3 or 6.
pub fn parse_hex_color(val: &str) -> Option<Color> {
    let s = val.strip_prefix('#').unwrap_or(val);
    match s.len() {
        3 => {
            let mut chars = s.chars();
            let r_char = chars.next()?;
            let g_char = chars.next()?;
            let b_char = chars.next()?;
            let r_str = format!("{r_char}{r_char}");
            let g_str = format!("{g_char}{g_char}");
            let b_str = format!("{b_char}{b_char}");
            let r = u8::from_str_radix(&r_str, 16).ok()?;
            let g = u8::from_str_radix(&g_str, 16).ok()?;
            let b = u8::from_str_radix(&b_str, 16).ok()?;
            Some(Color::Rgb { r, g, b })
        }
        6 => {
            let mut chars = s.chars();
            let r1 = chars.next()?;
            let r2 = chars.next()?;
            let g1 = chars.next()?;
            let g2 = chars.next()?;
            let b1 = chars.next()?;
            let b2 = chars.next()?;
            let r_str = format!("{r1}{r2}");
            let g_str = format!("{g1}{g2}");
            let b_str = format!("{b1}{b2}");
            let r = u8::from_str_radix(&r_str, 16).ok()?;
            let g = u8::from_str_radix(&g_str, 16).ok()?;
            let b = u8::from_str_radix(&b_str, 16).ok()?;
            Some(Color::Rgb { r, g, b })
        }
        _ => None,
    }
}

/// Parses `w:gridSpan` column span value (ECMA-376 §17.4.17).
///
/// The `w:gridSpan` attribute specifies the number of grid columns spanned by a table cell.
/// Per OOXML, the default is 1 (when the element is omitted). This function returns:
/// - `None` if the element is omitted (input is `None`)
/// - `None` if the value is "1" (explicit default, semantically equivalent to omitted)
/// - `Some(n)` if the value parses as a u32 and n > 1
/// - `None` for invalid, zero, or negative values (lenient; matches existing parser philosophy)
///
/// This mapping aligns with the `colspan: Option<u32>` field semantics: `None` means "use default (1)".
pub fn parse_grid_span_value(val: Option<&str>) -> Option<u32> {
    let s = val?;
    match s.parse::<u32>() {
        Ok(n) if n > 1 => Some(n),
        _ => None,
    }
}

/// Parses the `w:val` attribute of a `<w:color>` element.
///
/// Returns `None` for absent attribute, `"auto"`, or non-hex values.
pub fn parse_color_val(val: Option<&str>) -> Option<Color> {
    parse_hex_color(val?)
}

/// Highlight color palette (OOXML standard 16 colors, excluding "none").
const HIGHLIGHT_PALETTE: &[(&str, u8, u8, u8)] = &[
    ("black", 0, 0, 0),
    ("blue", 0, 0, 255),
    ("cyan", 0, 255, 255),
    ("green", 0, 255, 0),
    ("magenta", 255, 0, 255),
    ("red", 255, 0, 0),
    ("yellow", 255, 255, 0),
    ("white", 255, 255, 255),
    ("darkBlue", 0, 0, 128),
    ("darkCyan", 0, 128, 128),
    ("darkGreen", 0, 128, 0),
    ("darkMagenta", 128, 0, 128),
    ("darkRed", 128, 0, 0),
    ("darkYellow", 128, 128, 0),
    ("darkGray", 128, 128, 128),
    ("lightGray", 192, 192, 192),
];

/// Parses the `w:val` attribute of a `<w:highlight>` element.
///
/// Returns `None` for absent attribute, `"none"`, or unknown names.
pub fn parse_highlight_val(val: Option<&str>) -> Option<Color> {
    let s = val?;
    if s == "none" {
        return None;
    }
    HIGHLIGHT_PALETTE
        .iter()
        .find(|(name, _, _, _)| *name == s)
        .map(|(_, r, g, b)| Color::Rgb {
            r: *r,
            g: *g,
            b: *b,
        })
}

/// Parses the `w:fill` attribute of a `<w:shd>` element.
///
/// Returns `None` for absent attribute, `"auto"`, or non-hex values.
/// The `w:val` pattern attribute is intentionally ignored.
pub fn parse_shd_fill(fill: Option<&str>) -> Option<Color> {
    parse_hex_color(fill?)
}

/// Parse a `w:char` attribute value (hex string, typically 4 digits, u16 range)
/// into a u8 table-lookup key, applying Word's PUA stripping convention.
///
/// Returns `None` for unparseable input or for codepoints that fall outside
/// the 0x00-0xFF lookup range after PUA stripping.
pub(crate) fn parse_sym_char(hex: &str) -> Option<u8> {
    let raw = u16::from_str_radix(hex, 16).ok()?;
    let stripped = raw.checked_sub(0xF000).unwrap_or(raw);
    u8::try_from(stripped).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // parse_on_off tests (8 cases)
    // ============================================================================

    #[test]
    fn parse_on_off_none_returns_true() {
        assert!(parse_on_off(None));
    }

    #[test]
    fn parse_on_off_true_returns_true() {
        assert!(parse_on_off(Some("true")));
    }

    #[test]
    fn parse_on_off_one_returns_true() {
        assert!(parse_on_off(Some("1")));
    }

    #[test]
    fn parse_on_off_on_returns_true() {
        assert!(parse_on_off(Some("on")));
    }

    #[test]
    fn parse_on_off_false_returns_false() {
        assert!(!parse_on_off(Some("false")));
    }

    #[test]
    fn parse_on_off_zero_returns_false() {
        assert!(!parse_on_off(Some("0")));
    }

    #[test]
    fn parse_on_off_off_returns_false() {
        assert!(!parse_on_off(Some("off")));
    }

    #[test]
    fn parse_on_off_unknown_returns_true() {
        assert!(parse_on_off(Some("unknown")));
    }

    // ============================================================================
    // parse_underline_on tests (4 cases)
    // ============================================================================

    #[test]
    fn parse_underline_on_none_returns_false() {
        assert!(!parse_underline_on(None));
    }

    #[test]
    fn parse_underline_on_none_value_returns_false() {
        assert!(!parse_underline_on(Some("none")));
    }

    #[test]
    fn parse_underline_on_single_returns_true() {
        assert!(parse_underline_on(Some("single")));
    }

    #[test]
    fn parse_underline_on_double_returns_true() {
        assert!(parse_underline_on(Some("double")));
    }

    // ============================================================================
    // parse_vert_align tests (5 cases)
    // ============================================================================

    #[test]
    fn parse_vert_align_subscript_returns_subscript() {
        assert_eq!(parse_vert_align(Some("subscript")), VertAlign::Subscript);
    }

    #[test]
    fn parse_vert_align_superscript_returns_superscript() {
        assert_eq!(
            parse_vert_align(Some("superscript")),
            VertAlign::Superscript
        );
    }

    #[test]
    fn parse_vert_align_baseline_returns_none() {
        assert_eq!(parse_vert_align(Some("baseline")), VertAlign::None);
    }

    #[test]
    fn parse_vert_align_none_returns_none() {
        assert_eq!(parse_vert_align(None), VertAlign::None);
    }

    #[test]
    fn parse_vert_align_unknown_returns_none() {
        assert_eq!(parse_vert_align(Some("unknown")), VertAlign::None);
    }

    // ============================================================================
    // parse_alignment tests (8 cases)
    // ============================================================================

    #[test]
    fn parse_alignment_left_returns_left() {
        assert_eq!(parse_alignment("left"), Some(TextAlignment::Left));
    }

    #[test]
    fn parse_alignment_start_returns_left() {
        assert_eq!(parse_alignment("start"), Some(TextAlignment::Left));
    }

    #[test]
    fn parse_alignment_right_returns_right() {
        assert_eq!(parse_alignment("right"), Some(TextAlignment::Right));
    }

    #[test]
    fn parse_alignment_end_returns_right() {
        assert_eq!(parse_alignment("end"), Some(TextAlignment::Right));
    }

    #[test]
    fn parse_alignment_center_returns_center() {
        assert_eq!(parse_alignment("center"), Some(TextAlignment::Center));
    }

    #[test]
    fn parse_alignment_both_returns_justify() {
        assert_eq!(parse_alignment("both"), Some(TextAlignment::Justify));
    }

    #[test]
    fn parse_alignment_distribute_returns_justify() {
        assert_eq!(parse_alignment("distribute"), Some(TextAlignment::Justify));
    }

    #[test]
    fn parse_alignment_unknown_returns_none() {
        assert_eq!(parse_alignment("mediumKashida"), None);
    }

    // ============================================================================
    // parse_hex_color tests (11 cases)
    // ============================================================================

    #[test]
    fn parse_hex_color_six_char_uppercase() {
        assert_eq!(
            parse_hex_color("FF0000"),
            Some(Color::Rgb { r: 255, g: 0, b: 0 })
        );
    }

    #[test]
    fn parse_hex_color_six_char_lowercase() {
        assert_eq!(
            parse_hex_color("ff0000"),
            Some(Color::Rgb { r: 255, g: 0, b: 0 })
        );
    }

    #[test]
    fn parse_hex_color_six_char_mixed_case() {
        assert_eq!(
            parse_hex_color("Ff00aA"),
            Some(Color::Rgb {
                r: 255,
                g: 0,
                b: 170
            })
        );
    }

    #[test]
    fn parse_hex_color_three_char_expanded() {
        assert_eq!(
            parse_hex_color("f0f"),
            Some(Color::Rgb {
                r: 255,
                g: 0,
                b: 255
            })
        );
    }

    #[test]
    fn parse_hex_color_three_char_uppercase() {
        assert_eq!(
            parse_hex_color("ABC"),
            Some(Color::Rgb {
                r: 170,
                g: 187,
                b: 204
            })
        );
    }

    #[test]
    fn parse_hex_color_strips_leading_hash() {
        assert_eq!(
            parse_hex_color("#FF0000"),
            Some(Color::Rgb { r: 255, g: 0, b: 0 })
        );
    }

    #[test]
    fn parse_hex_color_auto_returns_none() {
        assert_eq!(parse_hex_color("auto"), None);
    }

    #[test]
    fn parse_hex_color_empty_returns_none() {
        assert_eq!(parse_hex_color(""), None);
    }

    #[test]
    fn parse_hex_color_invalid_length_returns_none() {
        assert_eq!(parse_hex_color("FFFF"), None);
        assert_eq!(parse_hex_color("FFFFF"), None);
        assert_eq!(parse_hex_color("FFFFFFF"), None);
    }

    #[test]
    fn parse_hex_color_non_hex_returns_none() {
        assert_eq!(parse_hex_color("GG0000"), None);
    }

    #[test]
    fn parse_hex_color_black_returns_some() {
        assert_eq!(
            parse_hex_color("000000"),
            Some(Color::Rgb { r: 0, g: 0, b: 0 })
        );
    }

    // ============================================================================
    // parse_color_val tests (6 cases)
    // ============================================================================

    #[test]
    fn parse_color_val_none_input_returns_none() {
        assert_eq!(parse_color_val(None), None);
    }

    #[test]
    fn parse_color_val_auto_returns_none() {
        assert_eq!(parse_color_val(Some("auto")), None);
    }

    #[test]
    fn parse_color_val_valid_hex_returns_color() {
        assert_eq!(
            parse_color_val(Some("FF0000")),
            Some(Color::Rgb { r: 255, g: 0, b: 0 })
        );
    }

    #[test]
    fn parse_color_val_invalid_hex_returns_none() {
        assert_eq!(parse_color_val(Some("xyz")), None);
    }

    #[test]
    fn parse_color_val_black_returns_some() {
        assert_eq!(
            parse_color_val(Some("000000")),
            Some(Color::Rgb { r: 0, g: 0, b: 0 })
        );
    }

    #[test]
    fn parse_color_val_with_leading_hash() {
        assert_eq!(
            parse_color_val(Some("#FF0000")),
            Some(Color::Rgb { r: 255, g: 0, b: 0 })
        );
    }

    // ============================================================================
    // parse_highlight_val tests (8 cases)
    // ============================================================================

    #[test]
    fn parse_highlight_val_none_input_returns_none() {
        assert_eq!(parse_highlight_val(None), None);
    }

    #[test]
    fn parse_highlight_val_explicit_none_returns_none() {
        assert_eq!(parse_highlight_val(Some("none")), None);
    }

    #[test]
    fn parse_highlight_val_yellow_returns_yellow() {
        assert_eq!(
            parse_highlight_val(Some("yellow")),
            Some(Color::Rgb {
                r: 255,
                g: 255,
                b: 0
            })
        );
    }

    #[test]
    fn parse_highlight_val_dark_blue_returns_dark_blue() {
        assert_eq!(
            parse_highlight_val(Some("darkBlue")),
            Some(Color::Rgb { r: 0, g: 0, b: 128 })
        );
    }

    #[test]
    fn parse_highlight_val_light_gray_returns_light_gray() {
        assert_eq!(
            parse_highlight_val(Some("lightGray")),
            Some(Color::Rgb {
                r: 192,
                g: 192,
                b: 192
            })
        );
    }

    #[test]
    fn parse_highlight_val_unknown_returns_none() {
        assert_eq!(parse_highlight_val(Some("orangeMaize")), None);
    }

    #[test]
    fn parse_highlight_val_case_sensitive() {
        assert_eq!(parse_highlight_val(Some("YELLOW")), None);
    }

    #[test]
    fn parse_highlight_val_all_palette_entries() {
        // Test all 16 named entries + "none"
        let test_cases = vec![
            ("black", Some(Color::Rgb { r: 0, g: 0, b: 0 })),
            ("blue", Some(Color::Rgb { r: 0, g: 0, b: 255 })),
            (
                "cyan",
                Some(Color::Rgb {
                    r: 0,
                    g: 255,
                    b: 255,
                }),
            ),
            ("green", Some(Color::Rgb { r: 0, g: 255, b: 0 })),
            (
                "magenta",
                Some(Color::Rgb {
                    r: 255,
                    g: 0,
                    b: 255,
                }),
            ),
            ("red", Some(Color::Rgb { r: 255, g: 0, b: 0 })),
            (
                "yellow",
                Some(Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 0,
                }),
            ),
            (
                "white",
                Some(Color::Rgb {
                    r: 255,
                    g: 255,
                    b: 255,
                }),
            ),
            ("darkBlue", Some(Color::Rgb { r: 0, g: 0, b: 128 })),
            (
                "darkCyan",
                Some(Color::Rgb {
                    r: 0,
                    g: 128,
                    b: 128,
                }),
            ),
            ("darkGreen", Some(Color::Rgb { r: 0, g: 128, b: 0 })),
            (
                "darkMagenta",
                Some(Color::Rgb {
                    r: 128,
                    g: 0,
                    b: 128,
                }),
            ),
            ("darkRed", Some(Color::Rgb { r: 128, g: 0, b: 0 })),
            (
                "darkYellow",
                Some(Color::Rgb {
                    r: 128,
                    g: 128,
                    b: 0,
                }),
            ),
            (
                "darkGray",
                Some(Color::Rgb {
                    r: 128,
                    g: 128,
                    b: 128,
                }),
            ),
            (
                "lightGray",
                Some(Color::Rgb {
                    r: 192,
                    g: 192,
                    b: 192,
                }),
            ),
            ("none", None),
        ];
        for (name, expected) in test_cases {
            assert_eq!(
                parse_highlight_val(Some(name)),
                expected,
                "Failed for: {name}"
            );
        }
    }

    // ============================================================================
    // parse_shd_fill tests (5 cases)
    // ============================================================================

    #[test]
    fn parse_shd_fill_none_input_returns_none() {
        assert_eq!(parse_shd_fill(None), None);
    }

    #[test]
    fn parse_shd_fill_auto_returns_none() {
        assert_eq!(parse_shd_fill(Some("auto")), None);
    }

    #[test]
    fn parse_shd_fill_valid_hex_returns_color() {
        assert_eq!(
            parse_shd_fill(Some("FFFF00")),
            Some(Color::Rgb {
                r: 255,
                g: 255,
                b: 0
            })
        );
    }

    #[test]
    fn parse_shd_fill_invalid_hex_returns_none() {
        assert_eq!(parse_shd_fill(Some("xyz")), None);
    }

    #[test]
    fn parse_shd_fill_black_returns_some() {
        assert_eq!(
            parse_shd_fill(Some("000000")),
            Some(Color::Rgb { r: 0, g: 0, b: 0 })
        );
    }

    // ============================================================================
    // parse_sym_char tests (9 cases)
    // ============================================================================

    #[test]
    fn parse_sym_char_raw_two_digits() {
        assert_eq!(parse_sym_char("4E"), Some(0x4E));
    }

    #[test]
    fn parse_sym_char_raw_four_digits() {
        assert_eq!(parse_sym_char("004E"), Some(0x4E));
    }

    #[test]
    fn parse_sym_char_pua_stripped() {
        assert_eq!(parse_sym_char("F04E"), Some(0x4E));
    }

    #[test]
    fn parse_sym_char_pua_boundary() {
        assert_eq!(parse_sym_char("F000"), Some(0x00));
        assert_eq!(parse_sym_char("F0FF"), Some(0xFF));
    }

    #[test]
    fn parse_sym_char_above_pua_range() {
        assert_eq!(parse_sym_char("F100"), None); // 0xF100 - 0xF000 = 0x100, > u8::MAX
        assert_eq!(parse_sym_char("F1FF"), None);
    }

    #[test]
    fn parse_sym_char_below_pua_above_u8() {
        assert_eq!(parse_sym_char("0100"), None); // raw 0x100, doesn't fit u8
    }

    #[test]
    fn parse_sym_char_invalid_hex() {
        assert_eq!(parse_sym_char("ZZZZ"), None);
        assert_eq!(parse_sym_char(""), None);
        assert_eq!(parse_sym_char("GH"), None);
    }

    #[test]
    fn parse_sym_char_empty() {
        assert_eq!(parse_sym_char(""), None);
    }

    #[test]
    fn parse_sym_char_with_leading_zeros() {
        assert_eq!(parse_sym_char("00F0"), Some(0xF0));
    }

    // ============================================================================
    // parse_grid_span_value tests (8 cases)
    // ============================================================================

    #[test]
    fn parse_grid_span_value_none_returns_none() {
        assert_eq!(parse_grid_span_value(None), None);
    }

    #[test]
    fn parse_grid_span_value_explicit_one_returns_none() {
        assert_eq!(parse_grid_span_value(Some("1")), None);
    }

    #[test]
    fn parse_grid_span_value_two_returns_some_two() {
        assert_eq!(parse_grid_span_value(Some("2")), Some(2));
    }

    #[test]
    fn parse_grid_span_value_zero_returns_none() {
        assert_eq!(parse_grid_span_value(Some("0")), None);
    }

    #[test]
    fn parse_grid_span_value_non_numeric_returns_none() {
        assert_eq!(parse_grid_span_value(Some("abc")), None);
    }

    #[test]
    fn parse_grid_span_value_empty_string_returns_none() {
        assert_eq!(parse_grid_span_value(Some("")), None);
    }

    #[test]
    fn parse_grid_span_value_large_value_returns_some() {
        assert_eq!(parse_grid_span_value(Some("100")), Some(100));
    }

    #[test]
    fn parse_grid_span_value_negative_returns_none() {
        assert_eq!(parse_grid_span_value(Some("-1")), None);
    }
}
