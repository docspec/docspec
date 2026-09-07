//! Derives internal configuration and the TypeScript surface for the compiled
//! conversion directions.
//!
//! The cfgs are cfgs rather than marker features because Cargo features are
//! public and additive: `--features _writer` could be selected on its own and
//! the marker would then claim a writer was compiled in when none was. A cfg
//! cannot be set from outside the crate.

use std::{env, fs, path::Path};

/// `(feature environment variable, canonical format name)`.
const READERS: [(&str, &str); 3] = [
    ("CARGO_FEATURE_DOCX_READER", "docx"),
    ("CARGO_FEATURE_HTML_READER", "html"),
    ("CARGO_FEATURE_MARKDOWN_READER", "markdown"),
];
const WRITERS: [(&str, &str); 5] = [
    ("CARGO_FEATURE_BLOCKNOTE_WRITER", "blocknote"),
    ("CARGO_FEATURE_HTML_WRITER", "html"),
    ("CARGO_FEATURE_MARKDOWN_WRITER", "markdown"),
    ("CARGO_FEATURE_OXA_WRITER", "oxa"),
    ("CARGO_FEATURE_PANDOC_NATIVE_WRITER", "pandoc-native"),
];

fn selected(table: &[(&str, &str)]) -> Vec<String> {
    table
        .iter()
        .filter(|(variable, _)| env::var_os(variable).is_some())
        .map(|(_, name)| format!("\"{name}\""))
        .collect()
}

/// Renders a TypeScript union, or `never` when nothing is compiled in.
///
/// `never` is load-bearing: it makes `convert_stream` uncallable from
/// TypeScript in a package that has no reader or no writer, which is exactly
/// the truth. The export still exists so a JavaScript caller gets the
/// documented `UNSUPPORTED_INPUT_FORMAT` / `UNSUPPORTED_OUTPUT_FORMAT` error
/// rather than a bare `TypeError`.
fn union(names: &[String]) -> String {
    if names.is_empty() {
        String::from("never")
    } else {
        names.join(" | ")
    }
}

fn main() -> Result<(), Box<dyn core::error::Error>> {
    println!("cargo::rustc-check-cfg=cfg(conversion)");
    println!("cargo::rustc-check-cfg=cfg(reader)");
    println!("cargo::rustc-check-cfg=cfg(writer)");

    let readers = selected(&READERS);
    let writers = selected(&WRITERS);

    if !readers.is_empty() {
        println!("cargo::rustc-cfg=reader");
    }
    if !writers.is_empty() {
        println!("cargo::rustc-cfg=writer");
    }
    // A conversion needs both halves. `reader` and `writer` are emitted
    // separately so `convert_stream` can prove the impossible cases away with
    // an uninhabited match instead of a runtime error that cannot fire.
    if !readers.is_empty() && !writers.is_empty() {
        println!("cargo::rustc-cfg=conversion");
    }

    // The declarations are generated rather than hand-written so the published
    // .d.ts describes the formats this package actually compiled in. A fixed
    // string typed `from`/`to` as `string`, which discarded the one guarantee a
    // selective build can offer its callers at compile time.
    let declarations = format!(
        r#"export interface ReadAtSource {{
  readonly size: number;
  readAt(offset: number, length: number): Uint8Array;
}}
export type WriteChunk =
  | ((chunk: Uint8Array) => void)
  | {{ write(chunk: Uint8Array): void }};
export type InputFormat = {input};
export type OutputFormat = {output};
export type DocspecErrorCode =
  | "INVALID_ARGUMENT"
  | "UNSUPPORTED_INPUT_FORMAT"
  | "UNSUPPORTED_OUTPUT_FORMAT"
  | "IO_ERROR"
  | "CONVERSION_ERROR";
export interface DocspecError extends Error {{
  readonly code: DocspecErrorCode;
}}
export function convert_stream(
  from: InputFormat,
  to: OutputFormat,
  source: ReadAtSource,
  write: WriteChunk,
): void;
"#,
        input = union(&readers),
        output = union(&writers),
    );

    let out_dir = env::var("OUT_DIR")?;
    fs::write(Path::new(&out_dir).join("api.d.ts"), declarations)?;
    Ok(())
}
