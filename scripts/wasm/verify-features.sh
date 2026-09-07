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
check_selection() {
  local selection="$1"
  if [[ -z "${selection}" ]]; then
    cargo clippy --locked --target wasm32-unknown-unknown \
      -p docspec-wasm --lib --no-default-features
  else
    cargo clippy --locked --target wasm32-unknown-unknown \
      -p docspec-wasm --lib --no-default-features --features "${selection}"
  fi
}

check_selection ""
for selection in "${individual_features[@]}" "${format_bundles[@]}" "${aggregate_features[@]}"; do
  check_selection "${selection}"
done
cargo clippy --locked --target wasm32-unknown-unknown -p docspec-wasm --lib
check_selection "docx-reader,markdown-writer"

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
