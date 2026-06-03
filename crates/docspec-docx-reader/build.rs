//! Build configuration for `docspec-docx-reader`.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(coverage)");
}
