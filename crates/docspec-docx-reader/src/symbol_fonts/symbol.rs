//! Symbol font codepoint mapping table.
//!
//! Maps Symbol font codepoints (0x20–0xFE) to Unicode characters.
//!
//! Source: <https://en.wikipedia.org/wiki/Symbol_(typeface)>
//! Last updated: 2026-06-10.

/// Maps Symbol font codepoints to Unicode characters.
///
/// Sorted by key (first tuple field). Covers range 0x20–0xFE.
pub(super) const TABLE: &[(u8, char)] = &[
    (0x20, '\u{00A0}'), // NO-BREAK SPACE
    (0x21, '!'),        // EXCLAMATION MARK
    (0x22, '\u{2200}'), // FOR ALL
    (0x23, '#'),        // NUMBER SIGN
    (0x24, '\u{2203}'), // THERE EXISTS
    (0x25, '%'),        // PERCENT SIGN
    (0x26, '&'),        // AMPERSAND
    (0x27, '\u{220b}'), // CONTAINS AS MEMBER
    (0x28, '('),        // LEFT PARENTHESIS
    (0x29, ')'),        // RIGHT PARENTHESIS
    (0x2A, '\u{2217}'), // ASTERISK OPERATOR
    (0x2B, '+'),        // PLUS SIGN
    (0x2C, ','),        // COMMA
    (0x2D, '\u{2212}'), // MINUS SIGN
    (0x2E, '.'),        // FULL STOP
    (0x2F, '/'),        // SOLIDUS
    (0x30, '0'),        // DIGIT ZERO
    (0x31, '1'),        // DIGIT ONE
    (0x32, '2'),        // DIGIT TWO
    (0x33, '3'),        // DIGIT THREE
    (0x34, '4'),        // DIGIT FOUR
    (0x35, '5'),        // DIGIT FIVE
    (0x36, '6'),        // DIGIT SIX
    (0x37, '7'),        // DIGIT SEVEN
    (0x38, '8'),        // DIGIT EIGHT
    (0x39, '9'),        // DIGIT NINE
    (0x3A, ':'),        // COLON
    (0x3B, ';'),        // SEMICOLON
    (0x3C, '<'),        // LESS-THAN SIGN
    (0x3D, '='),        // EQUALS SIGN
    (0x3E, '>'),        // GREATER-THAN SIGN
    (0x3F, '?'),        // QUESTION MARK
    (0x40, '\u{2245}'), // APPROXIMATELY EQUAL TO
    (0x41, '\u{391}'),  // GREEK CAPITAL LETTER ALPHA
    (0x42, '\u{392}'),  // GREEK CAPITAL LETTER BETA
    (0x43, '\u{3a7}'),  // GREEK CAPITAL LETTER CHI
    (0x44, '\u{2206}'), // INCREMENT
    (0x45, '\u{395}'),  // GREEK CAPITAL LETTER EPSILON
    (0x46, '\u{3a6}'),  // GREEK CAPITAL LETTER PHI
    (0x47, '\u{393}'),  // GREEK CAPITAL LETTER GAMMA
    (0x48, '\u{397}'),  // GREEK CAPITAL LETTER ETA
    (0x49, '\u{399}'),  // GREEK CAPITAL LETTER IOTA
    (0x4A, '\u{3d1}'),  // GREEK SMALL LETTER VARTHETA
    (0x4B, '\u{39a}'),  // GREEK CAPITAL LETTER KAPPA
    (0x4C, '\u{39b}'),  // GREEK CAPITAL LETTER LAMDA
    (0x4D, '\u{39c}'),  // GREEK CAPITAL LETTER MU
    (0x4E, '\u{39d}'),  // GREEK CAPITAL LETTER NU
    (0x4F, '\u{39f}'),  // GREEK CAPITAL LETTER OMICRON
    (0x50, '\u{3a0}'),  // GREEK CAPITAL LETTER PI
    (0x51, '\u{398}'),  // GREEK CAPITAL LETTER THETA
    (0x52, '\u{3a1}'),  // GREEK CAPITAL LETTER RHO
    (0x53, '\u{3a3}'),  // GREEK CAPITAL LETTER SIGMA
    (0x54, '\u{3a4}'),  // GREEK CAPITAL LETTER TAU
    (0x55, '\u{3a5}'),  // GREEK CAPITAL LETTER UPSILON
    (0x56, '\u{3c2}'),  // GREEK SMALL LETTER FINAL SIGMA
    (0x57, '\u{3a9}'),  // GREEK CAPITAL LETTER OMEGA
    (0x58, '\u{39e}'),  // GREEK CAPITAL LETTER XI
    (0x59, '\u{3a8}'),  // GREEK CAPITAL LETTER PSI
    (0x5A, '\u{396}'),  // GREEK CAPITAL LETTER ZETA
    (0x5B, '['),        // LEFT SQUARE BRACKET
    (0x5C, '\u{2234}'), // THEREFORE
    (0x5D, ']'),        // RIGHT SQUARE BRACKET
    (0x5E, '\u{22a5}'), // UP TACK
    (0x5F, '_'),        // LOW LINE
    (0x60, '\u{F8E5}'), // PRIVATE USE CHARACTER
    (0x61, '\u{3b1}'),  // GREEK SMALL LETTER ALPHA
    (0x62, '\u{3b2}'),  // GREEK SMALL LETTER BETA
    (0x63, '\u{3c7}'),  // GREEK SMALL LETTER CHI
    (0x64, '\u{3b4}'),  // GREEK SMALL LETTER DELTA
    (0x65, '\u{3b5}'),  // GREEK SMALL LETTER EPSILON
    (0x66, '\u{3c6}'),  // GREEK SMALL LETTER PHI
    (0x67, '\u{3b3}'),  // GREEK SMALL LETTER GAMMA
    (0x68, '\u{3b7}'),  // GREEK SMALL LETTER ETA
    (0x69, '\u{3b9}'),  // GREEK SMALL LETTER IOTA
    (0x6A, '\u{3d5}'),  // GREEK SMALL LETTER VARPHI
    (0x6B, '\u{3ba}'),  // GREEK SMALL LETTER KAPPA
    (0x6C, '\u{3bb}'),  // GREEK SMALL LETTER LAMDA
    (0x6D, '\u{3bc}'),  // GREEK SMALL LETTER MU
    (0x6E, '\u{3bd}'),  // GREEK SMALL LETTER NU
    (0x6F, '\u{3bf}'),  // GREEK SMALL LETTER OMICRON
    (0x70, '\u{3c0}'),  // GREEK SMALL LETTER PI
    (0x71, '\u{3b8}'),  // GREEK SMALL LETTER THETA
    (0x72, '\u{3c1}'),  // GREEK SMALL LETTER RHO
    (0x73, '\u{3c3}'),  // GREEK SMALL LETTER SIGMA
    (0x74, '\u{3c4}'),  // GREEK SMALL LETTER TAU
    (0x75, '\u{3c5}'),  // GREEK SMALL LETTER UPSILON
    (0x76, '\u{3d6}'),  // GREEK SMALL LETTER VARPI
    (0x77, '\u{3c9}'),  // GREEK SMALL LETTER OMEGA
    (0x78, '\u{3be}'),  // GREEK SMALL LETTER XI
    (0x79, '\u{3c8}'),  // GREEK SMALL LETTER PSI
    (0x7A, '\u{3b6}'),  // GREEK SMALL LETTER ZETA
    (0x7B, '{'),        // LEFT CURLY BRACKET
    (0x7C, '|'),        // VERTICAL LINE
    (0x7D, '}'),        // RIGHT CURLY BRACKET
    (0x7E, '\u{223c}'), // TILDE OPERATOR
    (0xA0, '\u{20ac}'), // EURO SIGN
    (0xA1, '\u{3d2}'),  // GREEK CAPITAL LETTER UPSILON WITH HOOK
    (0xA2, '\u{2032}'), // PRIME
    (0xA3, '\u{2264}'), // LESS-THAN OR EQUAL TO
    (0xA4, '\u{2044}'), // FRACTION SLASH
    (0xA5, '\u{221e}'), // INFINITY
    (0xA6, '\u{192}'),  // LATIN SMALL LETTER F WITH HOOK
    (0xA7, '\u{2663}'), // BLACK CLUB SUIT
    (0xA8, '\u{2666}'), // BLACK DIAMOND SUIT
    (0xA9, '\u{2665}'), // BLACK HEART SUIT
    (0xAA, '\u{2660}'), // BLACK SPADE SUIT
    (0xAB, '\u{2194}'), // LEFT RIGHT ARROW
    (0xAC, '\u{2190}'), // LEFTWARDS ARROW
    (0xAD, '\u{2191}'), // UPWARDS ARROW
    (0xAE, '\u{2192}'), // RIGHTWARDS ARROW
    (0xAF, '\u{2193}'), // DOWNWARDS ARROW
    (0xB0, '\u{b0}'),   // DEGREE SIGN
    (0xB1, '\u{b1}'),   // PLUS-MINUS SIGN
    (0xB2, '\u{2033}'), // DOUBLE PRIME
    (0xB3, '\u{2265}'), // GREATER-THAN OR EQUAL TO
    (0xB4, '\u{d7}'),   // MULTIPLICATION SIGN
    (0xB5, '\u{221d}'), // PROPORTIONAL TO
    (0xB6, '\u{2202}'), // PARTIAL DIFFERENTIAL
    (0xB7, '\u{2022}'), // BULLET
    (0xB8, '\u{f7}'),   // DIVISION SIGN
    (0xB9, '\u{2260}'), // NOT EQUAL TO
    (0xBA, '\u{2261}'), // IDENTICAL TO
    (0xBB, '\u{2248}'), // ALMOST EQUAL TO
    (0xBC, '\u{2026}'), // HORIZONTAL ELLIPSIS
    (0xBD, '\u{F8E6}'), // PRIVATE USE CHARACTER
    (0xBE, '\u{F8E7}'), // PRIVATE USE CHARACTER
    (0xBF, '\u{21b5}'), // DOWNWARDS ARROW WITH CORNER LEFTWARDS
    (0xC0, '\u{2135}'), // ALEF SYMBOL
    (0xC1, '\u{2111}'), // BLACK-LETTER CAPITAL I
    (0xC2, '\u{211c}'), // BLACK-LETTER CAPITAL R
    (0xC3, '\u{2118}'), // WEIERSTRASS ELLIPTIC FUNCTION
    (0xC4, '\u{2297}'), // CIRCLED TIMES
    (0xC5, '\u{2295}'), // CIRCLED PLUS
    (0xC6, '\u{2205}'), // EMPTY SET
    (0xC7, '\u{2229}'), // INTERSECTION
    (0xC8, '\u{222a}'), // UNION
    (0xC9, '\u{2283}'), // SUPERSET OF
    (0xCA, '\u{2287}'), // SUPERSET OF OR EQUAL TO
    (0xCB, '\u{2284}'), // NOT A SUBSET OF
    (0xCC, '\u{2282}'), // SUBSET OF
    (0xCD, '\u{2286}'), // SUBSET OF OR EQUAL TO
    (0xCE, '\u{2208}'), // ELEMENT OF
    (0xCF, '\u{2209}'), // NOT AN ELEMENT OF
    (0xD0, '\u{2220}'), // ANGLE
    (0xD1, '\u{2207}'), // NABLA
    (0xD2, '\u{F6DA}'), // PRIVATE USE CHARACTER
    (0xD3, '\u{F6D9}'), // PRIVATE USE CHARACTER
    (0xD4, '\u{F6DB}'), // PRIVATE USE CHARACTER
    (0xD5, '\u{220f}'), // N-ARY PRODUCT
    (0xD6, '\u{221a}'), // SQUARE ROOT
    (0xD7, '\u{22c5}'), // DOT OPERATOR
    (0xD8, '\u{ac}'),   // NOT SIGN
    (0xD9, '\u{2227}'), // LOGICAL AND
    (0xDA, '\u{2228}'), // LOGICAL OR
    (0xDB, '\u{21d4}'), // LEFT RIGHT DOUBLE ARROW
    (0xDC, '\u{21d0}'), // LEFTWARDS DOUBLE ARROW
    (0xDD, '\u{21d1}'), // UPWARDS DOUBLE ARROW
    (0xDE, '\u{21d2}'), // RIGHTWARDS DOUBLE ARROW
    (0xDF, '\u{21d3}'), // DOWNWARDS DOUBLE ARROW
    (0xE0, '\u{25ca}'), // LOZENGE
    (0xE1, '\u{27e8}'), // MATHEMATICAL LEFT ANGLE BRACKET
    (0xE2, '\u{F8E8}'), // PRIVATE USE CHARACTER
    (0xE3, '\u{F8E9}'), // PRIVATE USE CHARACTER
    (0xE4, '\u{F8EA}'), // PRIVATE USE CHARACTER
    (0xE5, '\u{2211}'), // N-ARY SUMMATION
    (0xE6, '\u{F8EB}'), // PRIVATE USE CHARACTER
    (0xE7, '\u{F8EC}'), // PRIVATE USE CHARACTER
    (0xE8, '\u{F8ED}'), // PRIVATE USE CHARACTER
    (0xE9, '\u{F8EE}'), // PRIVATE USE CHARACTER
    (0xEA, '\u{F8EF}'), // PRIVATE USE CHARACTER
    (0xEB, '\u{F8F0}'), // PRIVATE USE CHARACTER
    (0xEC, '\u{F8F1}'), // PRIVATE USE CHARACTER
    (0xED, '\u{F8F2}'), // PRIVATE USE CHARACTER
    (0xEE, '\u{F8F3}'), // PRIVATE USE CHARACTER
    (0xEF, '\u{F8F4}'), // PRIVATE USE CHARACTER
    (0xF1, '\u{27e9}'), // MATHEMATICAL RIGHT ANGLE BRACKET
    (0xF2, '\u{222b}'), // INTEGRAL
    (0xF3, '\u{2320}'), // TOP HALF INTEGRAL
    (0xF4, '\u{F8F5}'), // PRIVATE USE CHARACTER
    (0xF5, '\u{2321}'), // BOTTOM HALF INTEGRAL
    (0xF6, '\u{F8F6}'), // PRIVATE USE CHARACTER
    (0xF7, '\u{F8F7}'), // PRIVATE USE CHARACTER
    (0xF8, '\u{F8F8}'), // PRIVATE USE CHARACTER
    (0xF9, '\u{F8F9}'), // PRIVATE USE CHARACTER
    (0xFA, '\u{F8FA}'), // PRIVATE USE CHARACTER
    (0xFB, '\u{F8FB}'), // PRIVATE USE CHARACTER
    (0xFC, '\u{F8FC}'), // PRIVATE USE CHARACTER
    (0xFD, '\u{F8FD}'), // PRIVATE USE CHARACTER
    (0xFE, '\u{F8FE}'), // PRIVATE USE CHARACTER
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
