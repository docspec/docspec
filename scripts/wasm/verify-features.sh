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

check_selection() {
  local selection="$1"
  if [[ -z "${selection}" ]]; then
    cargo check --locked --target wasm32-unknown-unknown \
      -p docspec-wasm --lib --no-default-features
  else
    cargo check --locked --target wasm32-unknown-unknown \
      -p docspec-wasm --lib --no-default-features --features "${selection}"
  fi
}

check_selection ""
for selection in "${individual_features[@]}" "${format_bundles[@]}" "${aggregate_features[@]}"; do
  check_selection "${selection}"
done
cargo check --locked --target wasm32-unknown-unknown -p docspec-wasm --lib
check_selection "docx-reader,markdown-writer"

graph_file="$(mktemp)"
trap 'rm -f "${graph_file}"' EXIT
cargo tree --locked --target wasm32-unknown-unknown -p docspec-wasm \
  --no-default-features --features docx-reader,markdown-writer \
  --edges normal --prefix none >"${graph_file}"

required_crates=(docspec docspec-core docspec-docx-reader docspec-markdown-writer)
excluded_crates=(
  docspec-blocknote-writer
  docspec-cli
  docspec-html-reader
  docspec-html-writer
  docspec-http
  docspec-markdown-reader
  docspec-oxa-writer
  docspec-pandoc-native-writer
  docspec-telemetry
  opentelemetry
  posthog-rs
  reqwest
  sentry
  tracing
  tracing-subscriber
)

for crate_name in "${required_crates[@]}"; do
  if ! grep -Eq "^${crate_name} v" "${graph_file}"; then
    echo "required crate missing from minimal dependency graph: ${crate_name}" >&2
    exit 1
  fi
done

for crate_name in "${excluded_crates[@]}"; do
  if grep -Eq "^${crate_name} v" "${graph_file}"; then
    echo "unselected crate present in minimal dependency graph: ${crate_name}" >&2
    exit 1
  fi
done

echo "verified empty, individual, bundle, aggregate, default, and minimal WASM feature selections"
