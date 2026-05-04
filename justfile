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

# Run coverage and enforce 99.5% floor
coverage:
    cargo llvm-cov --workspace --lcov --output-path lcov.info
    cargo llvm-cov report --fail-under-lines 99.5

# Run coverage and open HTML report
coverage-html:
    cargo llvm-cov --workspace --html
    cargo llvm-cov report --fail-under-lines 99.5

# Check spelling with typos
typos:
    typos

# Check TOML formatting
taplo:
    taplo fmt --check

# Clean build artifacts
clean:
    cargo clean
