//! Derives internal configuration for compiled conversion directions.

use std::env;

const READERS: [&str; 3] = [
    "CARGO_FEATURE_DOCX_READER",
    "CARGO_FEATURE_HTML_READER",
    "CARGO_FEATURE_MARKDOWN_READER",
];
const WRITERS: [&str; 5] = [
    "CARGO_FEATURE_BLOCKNOTE_WRITER",
    "CARGO_FEATURE_HTML_WRITER",
    "CARGO_FEATURE_MARKDOWN_WRITER",
    "CARGO_FEATURE_OXA_WRITER",
    "CARGO_FEATURE_PANDOC_NATIVE_WRITER",
];

fn main() {
    println!("cargo::rustc-check-cfg=cfg(conversion)");
    if READERS.iter().any(|name| env::var_os(name).is_some())
        && WRITERS.iter().any(|name| env::var_os(name).is_some())
    {
        println!("cargo::rustc-cfg=conversion");
    }
}
