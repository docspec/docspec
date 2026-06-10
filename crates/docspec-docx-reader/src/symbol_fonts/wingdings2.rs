//! Wingdings 2 codepoint mapping table.
//!
//! Maps Wingdings 2 font codepoints (0x21–0xF9) to standard Unicode characters.
//!
//! Source: <https://github.com/jgm/pandoc/issues/9220#issuecomment-1828263607>
//! Last updated: 2026-06-10.

/// Wingdings 2 codepoint to Unicode character mapping.
///
/// Sorted by key (u8 codepoint). Used for binary search lookup.
pub(super) const TABLE: &[(u8, char)] = &[
    (0x21, '\u{270a}'),  // WRITING HAND
    (0x22, '\u{270b}'),  // RAISED HAND
    (0x23, '\u{270c}'),  // VICTORY HAND
    (0x24, '\u{270d}'),  // WRITING HAND (ALT)
    (0x25, '\u{2704}'),  // SCISSORS
    (0x26, '\u{2700}'),  // UPPER BLADE SCISSORS
    (0x27, '\u{1f71e}'), // ALCHEMICAL SYMBOL FOR DISTILLATION
    (0x28, '\u{1f71d}'), // ALCHEMICAL SYMBOL FOR SUBLIMATION
    (0x29, '\u{1f5c5}'), // DOCUMENT WITH LINES
    (0x2A, '\u{1f5c6}'), // DOCUMENT WITH LINES AND ARROW
    (0x2B, '\u{1f5c7}'), // DOCUMENT WITH ARROW
    (0x2C, '\u{1f5c8}'), // DOCUMENT WITH MULTIPLE LINES
    (0x2D, '\u{1f5c9}'), // DOCUMENT WITH MULTIPLE LINES AND ARROW
    (0x2E, '\u{1f5ca}'), // DOCUMENT WITH MULTIPLE LINES AND ARROW (ALT)
    (0x2F, '\u{1f5cb}'), // DOCUMENT WITH MULTIPLE LINES (ALT)
    (0x30, '\u{1f5cc}'), // DOCUMENT WITH MULTIPLE LINES (ALT2)
    (0x31, '\u{1f5cd}'), // DOCUMENT WITH MULTIPLE LINES (ALT3)
    (0x32, '\u{1f4cb}'), // CLIPBOARD
    (0x33, '\u{1f5d1}'), // WASTEBASKET
    (0x34, '\u{1f5d4}'), // FILE CABINET DRAWER
    (0x35, '\u{1f5d5}'), // CARD INDEX DIVIDERS
    (0x36, '\u{1f5d6}'), // CARD INDEX DIVIDERS (ALT)
    (0x37, '\u{1f5d7}'), // CARD INDEX DIVIDERS (ALT2)
    (0x38, '\u{1f5d8}'), // CARD INDEX DIVIDERS (ALT3)
    (0x39, '\u{1f5ad}'), // LOWER LEFT PENCIL
    (0x3A, '\u{1f5af}'), // LOWER RIGHT PENCIL
    (0x3B, '\u{1f5b1}'), // LOWER LEFT PENCIL (ALT)
    (0x3C, '\u{1f592}'), // LOWER LEFT PENCIL (ALT2)
    (0x3D, '\u{1f593}'), // LOWER LEFT PENCIL (ALT3)
    (0x3E, '\u{1f598}'), // LOWER LEFT PENCIL (ALT4)
    (0x3F, '\u{1f599}'), // LOWER LEFT PENCIL (ALT5)
    (0x40, '\u{1f59a}'), // LOWER LEFT PENCIL (ALT6)
    (0x41, '\u{1f59b}'), // LOWER LEFT PENCIL (ALT7)
    (0x42, '\u{1f448}'), // LEFTWARDS BLACK ARROW
    (0x43, '\u{1f449}'), // RIGHTWARDS BLACK ARROW
    (0x44, '\u{1f59c}'), // LOWER LEFT PENCIL (ALT8)
    (0x45, '\u{1f59d}'), // LOWER LEFT PENCIL (ALT9)
    (0x46, '\u{1f59e}'), // LOWER LEFT PENCIL (ALT10)
    (0x47, '\u{1f59f}'), // LOWER LEFT PENCIL (ALT11)
    (0x48, '\u{1f5a0}'), // LOWER LEFT PENCIL (ALT12)
    (0x49, '\u{1f5a1}'), // LOWER LEFT PENCIL (ALT13)
    (0x4A, '\u{1f446}'), // UPWARDS BLACK ARROW
    (0x4B, '\u{1f447}'), // DOWNWARDS BLACK ARROW
    (0x4C, '\u{1f5a2}'), // LOWER LEFT PENCIL (ALT14)
    (0x4D, '\u{1f5a3}'), // LOWER LEFT PENCIL (ALT15)
    (0x4E, '\u{1f591}'), // LOWER LEFT PENCIL (ALT16)
    (0x4F, '\u{1f5b4}'), // LOWER LEFT PENCIL (ALT17)
    (0x50, '\u{2713}'),  // CHECK MARK
    (0x51, '\u{1f5b5}'), // LOWER LEFT PENCIL (ALT18)
    (0x52, '\u{2611}'),  // BALLOT BOX WITH CHECK
    (0x53, '\u{2612}'),  // BALLOT BOX WITH X
    (0x54, '\u{2612}'),  // BALLOT BOX WITH X (DUP)
    (0x55, '\u{2bbe}'),  // UPPER LEFT CURVED ARROW
    (0x56, '\u{2bbf}'),  // UPPER RIGHT CURVED ARROW
    (0x57, '\u{2a38}'),  // CIRCLED REVERSE SOLIDUS
    (0x58, '\u{2a38}'),  // CIRCLED REVERSE SOLIDUS (DUP)
    (0x59, '\u{1f669}'), // LOWER LEFT PENCIL (ALT19)
    (0x5A, '\u{1f674}'), // LOWER LEFT PENCIL (ALT20)
    (0x5B, '\u{1f672}'), // LOWER LEFT PENCIL (ALT21)
    (0x5C, '\u{1f673}'), // LOWER LEFT PENCIL (ALT22)
    (0x5D, '\u{203d}'),  // INTERROBANG
    (0x5E, '\u{1f679}'), // LOWER LEFT PENCIL (ALT23)
    (0x5F, '\u{1f67a}'), // LOWER LEFT PENCIL (ALT24)
    (0x60, '\u{1f67b}'), // LOWER LEFT PENCIL (ALT25)
    (0x61, '\u{1f666}'), // LOWER LEFT PENCIL (ALT26)
    (0x62, '\u{1f664}'), // LOWER LEFT PENCIL (ALT27)
    (0x63, '\u{1f665}'), // LOWER LEFT PENCIL (ALT28)
    (0x64, '\u{1f667}'), // LOWER LEFT PENCIL (ALT29)
    (0x65, '\u{1f67a}'), // LOWER LEFT PENCIL (ALT30)
    (0x66, '\u{1f668}'), // LOWER LEFT PENCIL (ALT31)
    (0x67, '\u{1f669}'), // LOWER LEFT PENCIL (ALT32)
    (0x68, '\u{1f67b}'), // LOWER LEFT PENCIL (ALT33)
    (0x69, '\u{24ff}'),  // CIRCLED DIGIT ZERO
    (0x6A, '\u{2460}'),  // CIRCLED DIGIT ONE
    (0x6B, '\u{2461}'),  // CIRCLED DIGIT TWO
    (0x6C, '\u{2462}'),  // CIRCLED DIGIT THREE
    (0x6D, '\u{2463}'),  // CIRCLED DIGIT FOUR
    (0x6E, '\u{2464}'),  // CIRCLED DIGIT FIVE
    (0x6F, '\u{2465}'),  // CIRCLED DIGIT SIX
    (0x70, '\u{2466}'),  // CIRCLED DIGIT SEVEN
    (0x71, '\u{2467}'),  // CIRCLED DIGIT EIGHT
    (0x72, '\u{2468}'),  // CIRCLED DIGIT NINE
    (0x73, '\u{2469}'),  // CIRCLED NUMBER TEN
    (0x74, '\u{24ff}'),  // CIRCLED DIGIT ZERO (ALT)
    (0x75, '\u{2776}'),  // DINGBAT CIRCLED SANS-SERIF DIGIT ONE
    (0x76, '\u{2777}'),  // DINGBAT CIRCLED SANS-SERIF DIGIT TWO
    (0x77, '\u{2778}'),  // DINGBAT CIRCLED SANS-SERIF DIGIT THREE
    (0x78, '\u{2779}'),  // DINGBAT CIRCLED SANS-SERIF DIGIT FOUR
    (0x79, '\u{277a}'),  // DINGBAT CIRCLED SANS-SERIF DIGIT FIVE
    (0x7A, '\u{277b}'),  // DINGBAT CIRCLED SANS-SERIF DIGIT SIX
    (0x7B, '\u{277c}'),  // DINGBAT CIRCLED SANS-SERIF DIGIT SEVEN
    (0x7C, '\u{277d}'),  // DINGBAT CIRCLED SANS-SERIF DIGIT EIGHT
    (0x7D, '\u{277e}'),  // DINGBAT CIRCLED SANS-SERIF DIGIT NINE
    (0x7E, '\u{277f}'),  // DINGBAT CIRCLED SANS-SERIF NUMBER TEN
    (0x80, '\u{2609}'),  // SUN
    (0x81, '\u{1f315}'), // FULL MOON
    (0x82, '\u{263d}'),  // FIRST QUARTER MOON
    (0x83, '\u{263e}'),  // LAST QUARTER MOON
    (0x84, '\u{2e3f}'),  // RAISED DOT
    (0x85, '\u{271d}'),  // LATIN CROSS
    (0x86, '\u{1f547}'), // LOWER LEFT PENCIL (ALT34)
    (0x87, '\u{1f55c}'), // LOWER LEFT PENCIL (ALT35)
    (0x88, '\u{1f55d}'), // LOWER LEFT PENCIL (ALT36)
    (0x89, '\u{1f55e}'), // LOWER LEFT PENCIL (ALT37)
    (0x8A, '\u{1f55f}'), // LOWER LEFT PENCIL (ALT38)
    (0x8B, '\u{1f560}'), // LOWER LEFT PENCIL (ALT39)
    (0x8C, '\u{1f561}'), // LOWER LEFT PENCIL (ALT40)
    (0x8D, '\u{1f562}'), // LOWER LEFT PENCIL (ALT41)
    (0x8E, '\u{1f563}'), // LOWER LEFT PENCIL (ALT42)
    (0x8F, '\u{1f564}'), // LOWER LEFT PENCIL (ALT43)
    (0x90, '\u{1f565}'), // LOWER LEFT PENCIL (ALT44)
    (0x91, '\u{1f566}'), // LOWER LEFT PENCIL (ALT45)
    (0x92, '\u{1f567}'), // LOWER LEFT PENCIL (ALT46)
    (0x93, '\u{1f668}'), // LOWER LEFT PENCIL (ALT47)
    (0x94, '\u{1f669}'), // LOWER LEFT PENCIL (ALT48)
    (0x95, '\u{2022}'),  // BULLET
    (0x96, '\u{25cf}'),  // BLACK CIRCLE
    (0x97, '\u{26ab}'),  // MEDIUM BLACK CIRCLE
    (0x98, '\u{2b24}'),  // BLACK LARGE CIRCLE
    (0x99, '\u{1f785}'), // LOWER LEFT PENCIL (ALT49)
    (0x9A, '\u{1f786}'), // LOWER LEFT PENCIL (ALT50)
    (0x9B, '\u{1f787}'), // LOWER LEFT PENCIL (ALT51)
    (0x9C, '\u{1f788}'), // LOWER LEFT PENCIL (ALT52)
    (0x9D, '\u{1f78a}'), // LOWER LEFT PENCIL (ALT53)
    (0x9E, '\u{29bf}'),  // CIRCLED BULLET
    (0x9F, '\u{25fe}'),  // BLACK SMALL SQUARE
    (0xA1, '\u{25fc}'),  // BLACK MEDIUM SQUARE
    (0xA2, '\u{2b1b}'),  // BLACK LARGE SQUARE
    (0xA3, '\u{2b1c}'),  // WHITE LARGE SQUARE
    (0xA4, '\u{1f791}'), // LOWER LEFT PENCIL (ALT54)
    (0xA5, '\u{1f792}'), // LOWER LEFT PENCIL (ALT55)
    (0xA6, '\u{1f793}'), // LOWER LEFT PENCIL (ALT56)
    (0xA7, '\u{1f794}'), // LOWER LEFT PENCIL (ALT57)
    (0xA8, '\u{25a3}'),  // SQUARE WITH HORIZONTAL FILL
    (0xA9, '\u{1f795}'), // LOWER LEFT PENCIL (ALT58)
    (0xAA, '\u{1f796}'), // LOWER LEFT PENCIL (ALT59)
    (0xAB, '\u{1f797}'), // LOWER LEFT PENCIL (ALT60)
    (0xAC, '\u{2b29}'),  // BLACK MEDIUM DIAMOND
    (0xAD, '\u{25ad}'),  // WHITE RECTANGLE
    (0xAE, '\u{25c6}'),  // BLACK DIAMOND
    (0xAF, '\u{25c7}'),  // WHITE DIAMOND
    (0xB0, '\u{1f79a}'), // LOWER LEFT PENCIL (ALT61)
    (0xB1, '\u{25c8}'),  // WHITE DIAMOND CONTAINING BLACK SMALL DIAMOND
    (0xB2, '\u{1f79b}'), // LOWER LEFT PENCIL (ALT62)
    (0xB3, '\u{1f79c}'), // LOWER LEFT PENCIL (ALT63)
    (0xB4, '\u{1f79d}'), // LOWER LEFT PENCIL (ALT64)
    (0xB5, '\u{2b2a}'),  // BLACK MEDIUM DIAMOND
    (0xB6, '\u{25a7}'),  // SQUARE WITH VERTICAL FILL
    (0xB7, '\u{2b2b}'),  // WHITE MEDIUM DIAMOND
    (0xB8, '\u{25ca}'),  // LOZENGE
    (0xB9, '\u{1f7a0}'), // LOWER LEFT PENCIL (ALT65)
    (0xBA, '\u{2596}'),  // LOWER LEFT QUADRANT CIRCULAR ARC
    (0xBB, '\u{2597}'),  // LOWER RIGHT QUADRANT CIRCULAR ARC
    (0xBC, '\u{2bca}'),  // LOWER LEFT QUADRANT CIRCULAR ARC
    (0xBD, '\u{2bcb}'),  // LOWER RIGHT QUADRANT CIRCULAR ARC
    (0xBE, '\u{25fc}'),  // BLACK MEDIUM SQUARE (DUP)
    (0xBF, '\u{25ad}'),  // WHITE RECTANGLE (DUP)
    (0xC0, '\u{2b1f}'),  // WHITE HEXAGON
    (0xC1, '\u{2bc2}'),  // LOWER LEFT QUADRANT CIRCULAR ARC
    (0xC2, '\u{2b23}'),  // WHITE HEXAGON (ALT)
    (0xC3, '\u{2b22}'),  // BLACK HEXAGON
    (0xC4, '\u{2bc3}'),  // LOWER LEFT QUADRANT CIRCULAR ARC
    (0xC5, '\u{2bc4}'),  // LOWER LEFT QUADRANT CIRCULAR ARC
    (0xC6, '\u{1f7a1}'), // LOWER LEFT PENCIL (ALT66)
    (0xC7, '\u{1f7a2}'), // LOWER LEFT PENCIL (ALT67)
    (0xC8, '\u{1f7a3}'), // LOWER LEFT PENCIL (ALT68)
    (0xC9, '\u{1f7a4}'), // LOWER LEFT PENCIL (ALT69)
    (0xCA, '\u{1f7a5}'), // LOWER LEFT PENCIL (ALT70)
    (0xCB, '\u{1f7a6}'), // LOWER LEFT PENCIL (ALT71)
    (0xCC, '\u{1f7a7}'), // LOWER LEFT PENCIL (ALT72)
    (0xCD, '\u{1f7a8}'), // LOWER LEFT PENCIL (ALT73)
    (0xCE, '\u{1f7a9}'), // LOWER LEFT PENCIL (ALT74)
    (0xCF, '\u{1f7aa}'), // LOWER LEFT PENCIL (ALT75)
    (0xD0, '\u{1f7ab}'), // LOWER LEFT PENCIL (ALT76)
    (0xD1, '\u{1f7ac}'), // LOWER LEFT PENCIL (ALT77)
    (0xD2, '\u{1f7ad}'), // LOWER LEFT PENCIL (ALT78)
    (0xD3, '\u{1f7ae}'), // LOWER LEFT PENCIL (ALT79)
    (0xD4, '\u{1f7af}'), // LOWER LEFT PENCIL (ALT80)
    (0xD5, '\u{1f7b0}'), // LOWER LEFT PENCIL (ALT81)
    (0xD6, '\u{1f7b1}'), // LOWER LEFT PENCIL (ALT82)
    (0xD7, '\u{1f7b2}'), // LOWER LEFT PENCIL (ALT83)
    (0xD8, '\u{1f7b3}'), // LOWER LEFT PENCIL (ALT84)
    (0xD9, '\u{1f7b4}'), // LOWER LEFT PENCIL (ALT85)
    (0xDA, '\u{1f7b5}'), // LOWER LEFT PENCIL (ALT86)
    (0xDB, '\u{1f7b6}'), // LOWER LEFT PENCIL (ALT87)
    (0xDC, '\u{1f7b7}'), // LOWER LEFT PENCIL (ALT88)
    (0xDD, '\u{1f7b8}'), // LOWER LEFT PENCIL (ALT89)
    (0xDE, '\u{1f7b9}'), // LOWER LEFT PENCIL (ALT90)
    (0xDF, '\u{1f7ba}'), // LOWER LEFT PENCIL (ALT91)
    (0xE0, '\u{1f7bb}'), // LOWER LEFT PENCIL (ALT92)
    (0xE1, '\u{1f7bc}'), // LOWER LEFT PENCIL (ALT93)
    (0xE2, '\u{1f7bd}'), // LOWER LEFT PENCIL (ALT94)
    (0xE3, '\u{1f7be}'), // LOWER LEFT PENCIL (ALT95)
    (0xE4, '\u{1f7bf}'), // LOWER LEFT PENCIL (ALT96)
    (0xE5, '\u{1f7c0}'), // LOWER LEFT PENCIL (ALT97)
    (0xE6, '\u{1f7c2}'), // LOWER LEFT PENCIL (ALT98)
    (0xE7, '\u{1f7c4}'), // LOWER LEFT PENCIL (ALT99)
    (0xE8, '\u{2726}'),  // BLACK FOUR POINTED STAR
    (0xE9, '\u{1f7c9}'), // LOWER LEFT PENCIL (ALT100)
    (0xEA, '\u{2605}'),  // BLACK STAR
    (0xEB, '\u{2736}'),  // SIX POINTED STAR
    (0xEC, '\u{1f7cb}'), // LOWER LEFT PENCIL (ALT101)
    (0xED, '\u{2737}'),  // EIGHT POINTED STAR
    (0xEE, '\u{1f7cf}'), // LOWER LEFT PENCIL (ALT102)
    (0xEF, '\u{1f7d2}'), // LOWER LEFT PENCIL (ALT103)
    (0xF0, '\u{2739}'),  // HEAVY TEARDROP SPOKED ASTERISK
    (0xF1, '\u{1f7c3}'), // LOWER LEFT PENCIL (ALT104)
    (0xF2, '\u{1f7c7}'), // LOWER LEFT PENCIL (ALT105)
    (0xF3, '\u{272f}'),  // HEAVY TEARDROP SPOKED ASTERISK (ALT)
    (0xF4, '\u{1f7cd}'), // LOWER LEFT PENCIL (ALT106)
    (0xF5, '\u{1f7d4}'), // LOWER LEFT PENCIL (ALT107)
    (0xF6, '\u{2bcc}'),  // LOWER LEFT QUADRANT CIRCULAR ARC
    (0xF7, '\u{2bcd}'),  // LOWER LEFT QUADRANT CIRCULAR ARC
    (0xF8, '\u{203b}'),  // REFERENCE MARK
    (0xF9, '\u{2042}'),  // ASTERISM
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_sorted() {
        for (prev, curr) in TABLE.iter().zip(TABLE.iter().skip(1)) {
            assert!(
                prev.0 < curr.0,
                "table not sorted: {:#04X} >= {:#04X}",
                prev.0,
                curr.0
            );
        }
    }

    #[test]
    fn table_no_duplicates() {
        for (prev, curr) in TABLE.iter().zip(TABLE.iter().skip(1)) {
            assert_ne!(prev.0, curr.0, "duplicate key: {:#04X}", curr.0);
        }
    }
}
