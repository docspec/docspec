# Default recipe: run all checks (what CI does)
default: fmt clippy test doc build

# Format all Rust code
fmt:
    cargo fmt --all

# Check formatting without modifying files
fmt-check:
    cargo fmt --all --check

# Run clippy with workspace-wide strict lints
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Build the entire workspace
build:
    cargo build --workspace

# Build in release mode
release:
    cargo build --workspace --release

# Run all tests
test:
    cargo test --workspace

# Check documentation builds without warnings
# To open in browser, run `just doc -- --open`
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Run coverage and produce a report.
# --ignore-filename-regex excludes binary/WASM entry points per TESTING.md tolerance:
# CLI and WASM entry points are integration-tested via smoke tests but cannot be covered
# by unit tests (no testable main() or wasm_bindgen entry points).
# Coverage is informational: new code is held to 98% in review; workspace totals are not gated.
coverage:
    cargo llvm-cov --workspace --lcov --output-path lcov.info \
        --ignore-filename-regex "src/bin/|docspec-wasm/src/lib\.rs"
    cargo llvm-cov report \
        --ignore-filename-regex "src/bin/|docspec-wasm/src/lib\.rs"

# Run coverage and open HTML report
coverage-html:
    cargo llvm-cov --workspace --html \
        --ignore-filename-regex "src/bin/|docspec-wasm/src/lib\.rs"
    cargo llvm-cov report \
        --ignore-filename-regex "src/bin/|docspec-wasm/src/lib\.rs"

# Check spelling with typos
typos:
    typos

# Check TOML formatting
taplo:
    taplo fmt --check

# Build a development WASM package. `just wasm` preserves the legacy browser package.
wasm target="web" features="markdown-reader,blocknote-writer" out_dir="pkg":
    wasm-pack build --dev --target {{ quote(target) }} --out-dir {{ quote(out_dir) }} crates/docspec-wasm -- --locked --no-default-features {{ if features == "" { "" } else { "--features " + quote(features) } }}

# Build an optimized WASM package. Pass a new `out_dir` for each distributable build.
wasm-release target="web" features="markdown-reader,blocknote-writer" out_dir="pkg":
    wasm-pack build --profile wasm-release --target {{ quote(target) }} --out-dir {{ quote(out_dir) }} crates/docspec-wasm -- --locked --no-default-features {{ if features == "" { "" } else { "--features " + quote(features) } }}

# Clean build artifacts
clean:
    cargo clean
