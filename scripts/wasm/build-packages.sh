#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_root="${1:-${repository_root}/target/wasm-pack}"

build_package() {
  local target="$1"
  local name="$2"
  local features="$3"
  local profile="${4:---profile wasm-release}"
  local output_dir="${output_root}/${target}-${name}"
  local feature_args=(--locked --no-default-features)
  # An empty selection is a supported build, but `--features ""` is not the way
  # to ask for one: cargo parses it as a feature named "". Omit the flag.
  if [[ -n "${features}" ]]; then
    feature_args+=(--features "${features}")
  fi

  rm -rf "${output_dir}"
  # shellcheck disable=SC2086 # profile is a deliberate multi-word flag pair
  wasm-pack build "${repository_root}/crates/docspec-wasm" \
    --target "${target}" \
    ${profile} \
    --out-dir "${output_dir}" \
    -- "${feature_args[@]}"

  test -s "${output_dir}/docspec_wasm_bg.wasm"
  test -s "${output_dir}/docspec_wasm.d.ts"
}

mkdir -p "${output_root}"
build_package nodejs minimal docx-reader,markdown-writer
build_package nodejs full full
build_package web docx-to-markdown docx-reader,markdown-writer
build_package web full full
# A no-feature package, built --dev because only its declarations are read. It
# is the only selection that exercises the `never` and narrowed-error-code
# branches of build.rs; every distributable package above compiles a conversion.
build_package web empty "" --dev

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

# The generated declarations must name exactly the compiled formats. This is the
# guarantee a selective build offers TypeScript callers, and a hand-written
# fixed string (`from: string`) silently gave it away.
grep -Fqx 'export type InputFormat = "docx";' "${minimal_types}"
grep -Fqx 'export type OutputFormat = "markdown";' "${minimal_types}"
grep -Fqx 'export type InputFormat = "docx" | "html" | "markdown";' "${full_types}"
grep -Fqx 'export type OutputFormat = "blocknote" | "html" | "markdown" | "oxa" | "pandoc-native";' "${full_types}"

# `DocspecErrorCode` narrows too. IO_ERROR and CONVERSION_ERROR are reachable
# only once a conversion can run, so the trailing `;` -- which lands on the last
# member of the union -- is what distinguishes the two shapes. The members are
# flush left because wasm-bindgen strips leading whitespace from a
# typescript_custom_section; build.rs emits them that way for the same reason.
grep -Fqx '| "CONVERSION_ERROR";' "${minimal_types}"
grep -Fqx '| "CONVERSION_ERROR";' "${full_types}"

empty_types="${output_root}/web-empty/docspec_wasm.d.ts"
grep -Fqx 'export type InputFormat = never;' "${empty_types}"
grep -Fqx 'export type OutputFormat = never;' "${empty_types}"
grep -Fqx '| "UNSUPPORTED_OUTPUT_FORMAT";' "${empty_types}"
if grep -Fq 'IO_ERROR' "${empty_types}"; then
  echo "empty package declares IO_ERROR, which it can never throw" >&2
  exit 1
fi
if grep -Fq 'CONVERSION_ERROR' "${empty_types}"; then
  echo "empty package declares CONVERSION_ERROR, which it can never throw" >&2
  exit 1
fi

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

# Absolute budgets. The relative check above passes even when BOTH artifacts
# inflate, which is precisely the regression a size-motivated feature must
# catch. gzip is the number users actually download, so gate on that.
#
# Measured 2026-09-07: minimal 153001, full 348520. Budgets carry ~10% headroom.
# Raising one is a deliberate act -- record why in the commit message.
: "${MINIMAL_GZIP_BUDGET:=168000}"
: "${FULL_GZIP_BUDGET:=383000}"
check_budget() {
  local name="$1" actual="$2" budget="$3"
  if (( actual > budget )); then
    echo "${name} gzip ${actual} exceeds budget ${budget} (+$(( actual - budget )) bytes)" >&2
    echo "investigate the growth, or raise the budget deliberately." >&2
    exit 1
  fi
  printf '  %s gzip %s / budget %s (%s bytes headroom)\n' \
    "${name}" "${actual}" "${budget}" "$(( budget - actual ))"
}
check_budget minimal "${minimal_gzip}" "${MINIMAL_GZIP_BUDGET}"
check_budget full "${full_gzip}" "${FULL_GZIP_BUDGET}"

# Byte-level selectivity. The cargo-tree assertions in verify-features.sh prove
# crate absence; this proves the compiled artifact carries no trace of a format
# that was not selected -- the literal promise of a selective build.
for absent in blocknote pandoc oxa; do
  if grep -c -a -o "${absent}" "${minimal_wasm}" >/dev/null 2>&1 \
     && [ "$(grep -c -a -o "${absent}" "${minimal_wasm}")" != "0" ]; then
    echo "minimal WASM contains '${absent}', which was not selected" >&2
    exit 1
  fi
done
for present in blocknote pandoc oxa; do
  if [ "$(grep -c -a -o "${present}" "${full_wasm}")" = "0" ]; then
    echo "full WASM is missing '${present}'; the selectivity probe is vacuous" >&2
    exit 1
  fi
done
echo "  byte-level selectivity verified (minimal excludes blocknote/pandoc/oxa; full includes them)"

printf 'minimal_raw=%s\nfull_raw=%s\nminimal_gzip=%s\nfull_gzip=%s\n' \
  "${minimal_raw}" "${full_raw}" "${minimal_gzip}" "${full_gzip}" \
  | tee "${output_root}/sizes.txt"
