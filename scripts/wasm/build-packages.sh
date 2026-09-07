#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_root="${1:-${repository_root}/target/wasm-pack}"

build_package() {
  local target="$1"
  local name="$2"
  local features="$3"
  local output_dir="${output_root}/${target}-${name}"

  rm -rf "${output_dir}"
  wasm-pack build "${repository_root}/crates/docspec-wasm" \
    --target "${target}" \
    --profile wasm-release \
    --out-dir "${output_dir}" \
    -- --locked --no-default-features --features "${features}"

  test -s "${output_dir}/docspec_wasm_bg.wasm"
  test -s "${output_dir}/docspec_wasm.d.ts"
}

mkdir -p "${output_root}"
build_package nodejs minimal docx-reader,markdown-writer
build_package nodejs full full
build_package web docx-to-markdown docx-reader,markdown-writer
build_package web full full

minimal_types="${output_root}/nodejs-minimal/docspec_wasm.d.ts"
full_types="${output_root}/nodejs-full/docspec_wasm.d.ts"
grep -Fq 'export interface ReadAtSource' "${minimal_types}"
grep -Fq 'export type WriteChunk' "${minimal_types}"
grep -Fq 'source: ReadAtSource' "${minimal_types}"
grep -Fq 'write: WriteChunk' "${minimal_types}"
if grep -Fq 'convert_markdown_to_blocknote' "${minimal_types}"; then
  echo "minimal package unexpectedly exports the legacy conversion" >&2
  exit 1
fi
grep -Fq 'convert_markdown_to_blocknote' "${full_types}"

minimal_wasm="${output_root}/nodejs-minimal/docspec_wasm_bg.wasm"
full_wasm="${output_root}/nodejs-full/docspec_wasm_bg.wasm"
minimal_raw="$(wc -c <"${minimal_wasm}")"
full_raw="$(wc -c <"${full_wasm}")"
minimal_gzip="$(gzip -9 -c "${minimal_wasm}" | wc -c)"
full_gzip="$(gzip -9 -c "${full_wasm}" | wc -c)"

if (( minimal_raw >= full_raw || minimal_gzip >= full_gzip )); then
  echo "minimal WASM must be smaller than full (raw ${minimal_raw}/${full_raw}, gzip ${minimal_gzip}/${full_gzip})" >&2
  exit 1
fi

printf 'minimal_raw=%s\nfull_raw=%s\nminimal_gzip=%s\nfull_gzip=%s\n' \
  "${minimal_raw}" "${full_raw}" "${minimal_gzip}" "${full_gzip}" \
  | tee "${output_root}/sizes.txt"
