#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repository_root}"

individual_features=(
  docx-reader
  html-reader
  markdown-reader
  blocknote-writer
  html-writer
  markdown-writer
  oxa-writer
  pandoc-native-writer
)
format_bundles=(docx html markdown blocknote oxa pandoc-native)
aggregate_features=(all-readers all-writers full)

# clippy, not check: the workspace lint set is strict, and the `clippy` CI job
# only ever lints the DEFAULT feature set. Reader-only and writer-only builds
# went unlinted entirely and had accumulated real violations that `cargo check`
# cannot see.
#
# --all-targets, not the library alone: build.rs and tests/wasm.rs are both
# feature-aware and neither was being built here. `tests/wasm.rs` derives its
# expected capability lists from the same cfgs the library uses, so it is the
# one target that can catch a name or ordering drift in a non-default
# selection -- and it was only ever compiled by `cargo test --workspace`, i.e.
# at the default features. The build script was linted nowhere at all.
check_selection() {
  local selection="$1"
  if [[ -z "${selection}" ]]; then
    cargo clippy --locked --target wasm32-unknown-unknown \
      -p docspec-wasm --all-targets --no-default-features
  else
    cargo clippy --locked --target wasm32-unknown-unknown \
      -p docspec-wasm --all-targets --no-default-features --features "${selection}"
  fi
}

check_selection ""
for selection in "${individual_features[@]}" "${format_bundles[@]}" "${aggregate_features[@]}"; do
  check_selection "${selection}"
done
cargo clippy --locked --target wasm32-unknown-unknown -p docspec-wasm --all-targets
check_selection "docx-reader,markdown-writer"

# Compiling tests/wasm.rs is not the same as running it. Its expected capability
# lists are a SECOND, hand-maintained copy of the same `#[cfg(feature = ...)]`
# arms that formats.rs uses, so a misspelled or mis-ordered format name only
# shows up when the two are actually compared at runtime.
#
# These run on the host, not wasm32: tests/wasm.rs is a native test (no
# wasm-bindgen-test), so `cargo test` executes it directly.
#
# Six selections rather than all eighteen, because what varies is the SHAPE.
# Empty, reader-only and writer-only cover the narrowing cases; `full` carries
# every individual format name and so catches a per-format typo wherever it is;
# minimal and default are the two selections that actually ship. The remaining
# individual features and bundles are structurally identical to one of these and
# are already compiled by the clippy sweep above.
run_selection() {
  local selection="$1"
  if [[ -z "${selection}" ]]; then
    cargo test --locked -p docspec-wasm --no-default-features
  else
    cargo test --locked -p docspec-wasm --no-default-features --features "${selection}"
  fi
}

run_selection ""
for selection in docx-reader oxa-writer docx-reader,markdown-writer \
  markdown-reader,blocknote-writer full; do
  run_selection "${selection}"
done

graph_file="$(mktemp)"
trap 'rm -f "${graph_file}"' EXIT

# Server, CLI, and telemetry crates must never reach any WASM artifact.
# ARCHITECTURE.md rests the docspec-http exclusion on tokio specifically, so
# assert the async runtime and its transport stack by name.
never_present=(
  docspec-cli
  docspec-http
  axum
  hyper
  posthog-rs
  reqwest
  sentry
  tokio
  tracing
  tracing-subscriber
)

# A denylist entry that names a crate no longer in Cargo.lock can never match,
# so it silently stops asserting anything. (This is not hypothetical: the list
# previously carried `docspec-telemetry` and `opentelemetry`, neither of which
# exists in this workspace.) Fail loudly instead of rotting quietly.
for crate_name in "${never_present[@]}"; do
  if ! grep -Eq "^name = \"${crate_name}\"$" "${repository_root}/Cargo.lock"; then
    echo "denylist entry is not in Cargo.lock and asserts nothing: ${crate_name}" >&2
    echo "remove it, or correct the name." >&2
    exit 1
  fi
done

# Dependencies that exist only to serve a direction we did not select, or that
# no read-only conversion can use. `zopfli` is a compressor and `getopts` is a
# CLI argument parser; both were once pulled in transitively for nothing.
#
# `zlib-rs` is asserted PRESENT wherever DOCX is selected, so a silent backend
# swap has to fail here rather than surface later as unexplained size drift. It
# is deliberately NOT paired with a miniz_oxide denial: flate2 declares that
# crate under `[target.'cfg(target_arch = "wasm32")'.dependencies]` as a hard,
# non-optional dependency, so it is in every wasm32 graph whichever backend the
# features select. Denying it would assert something false and fail here always.
assert_graph() {
  local selection="$1" required="$2" excluded="$3"
  cargo tree --locked --target wasm32-unknown-unknown -p docspec-wasm \
    --no-default-features --features "${selection}" \
    --edges normal --prefix none >"${graph_file}"

  local crate_name
  for crate_name in ${required}; do
    if ! grep -Eq "^${crate_name} v" "${graph_file}"; then
      echo "[${selection}] required crate missing from graph: ${crate_name}" >&2
      exit 1
    fi
  done
  for crate_name in ${excluded} "${never_present[@]}"; do
    if grep -Eq "^${crate_name} v" "${graph_file}"; then
      echo "[${selection}] unselected crate present in graph: ${crate_name}" >&2
      exit 1
    fi
  done
  echo "  graph verified: ${selection}"
}

# Minimal DOCX to Markdown: no markdown parser, no compressor, no other format.
assert_graph "docx-reader,markdown-writer" \
  "docspec docspec-core docspec-docx-reader docspec-markdown-writer zip flate2 quick-xml
   zlib-rs" \
  "docspec-blocknote-writer docspec-html-reader docspec-html-writer
   docspec-markdown-reader docspec-oxa-writer docspec-pandoc-native-writer
   docspec-json pulldown-cmark html5gum zopfli getopts"

# Default package: no DOCX stack at all.
assert_graph "markdown-reader,blocknote-writer" \
  "docspec docspec-core docspec-markdown-reader docspec-blocknote-writer docspec-json pulldown-cmark" \
  "docspec-docx-reader docspec-html-reader docspec-html-writer
   docspec-markdown-writer docspec-oxa-writer docspec-pandoc-native-writer
   zip flate2 quick-xml zopfli getopts zlib-rs"

# Full package: every format crate present, server and CLI still absent.
assert_graph "full" \
  "docspec docspec-core docspec-docx-reader docspec-html-reader
   docspec-markdown-reader docspec-blocknote-writer docspec-html-writer
   docspec-markdown-writer docspec-oxa-writer docspec-pandoc-native-writer
   zlib-rs" \
  "zopfli getopts"

echo "verified empty, individual, bundle, aggregate, default, and minimal WASM feature selections"
