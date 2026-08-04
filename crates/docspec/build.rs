//! Build configuration for `docspec`.
//!
//! Derives one cfg per "is any reader / writer compiled in" question, so each feature
//! list is written down once instead of at every use — the drift between two hand-copied
//! lists is what broke the non-default feature selections in the first place.
//!
//! These started out as Cargo features (`_reader`, `_writer`, `_text-reader`). Cargo
//! features are public and additive, so `--features _writer` could be selected on its own,
//! where the marker then claims a writer is compiled in when none is. A cfg derived here
//! cannot be set from the outside and cannot disagree with the features it is read from.

use std::env;

/// Each derived cfg and the features that imply it, spelled as Cargo puts them in the
/// environment: `CARGO_FEATURE_` followed by the feature name uppercased, `-` as `_`.
///
/// `text_reader` is a subset rather than a shorter spelling of `reader`: the BOM-stripping
/// reader wraps the text formats, and DOCX is binary.
const DERIVED_CFGS: [(&str, &[&str]); 3] = [
    ("reader", &["MARKDOWN", "HTML", "DOCX"]),
    ("text_reader", &["MARKDOWN", "HTML"]),
    (
        "writer",
        &[
            "BLOCKNOTE_WRITER",
            "OXA_WRITER",
            "HTML_WRITER",
            "PANDOC_NATIVE_WRITER",
            "MARKDOWN_WRITER",
        ],
    ),
];

fn main() {
    for (cfg, features) in DERIVED_CFGS {
        println!("cargo::rustc-check-cfg=cfg({cfg})");
        if features
            .iter()
            .any(|feature| env::var_os(format!("CARGO_FEATURE_{feature}")).is_some())
        {
            println!("cargo::rustc-cfg={cfg}");
        }
    }
}
