#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_dir="${1:-${repository_root}/target/wasm-memory-fixtures}"
base_docx="${repository_root}/tests/fixtures/docx/docspec/preformatted-boundaries.docx"

rm -rf "${output_dir}"
mkdir -p "${output_dir}"

for paragraph_count in 100 1000 10000 100000; do
  work_dir="$(mktemp -d)"
  unzip -q "${base_docx}" -d "${work_dir}"
  awk -v count="${paragraph_count}" 'BEGIN {
    print "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>"
    print "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>"
    for (i = 0; i < count; i++) {
      printf "<w:p><w:r><w:t>bounded paragraph %08d</w:t></w:r></w:p>\n", i
    }
    print "</w:body></w:document>"
  }' >"${work_dir}/word/document.xml"
  find "${work_dir}" -exec touch -t 198001010000 {} +
  (
    cd "${work_dir}"
    zip -X -q -r "${output_dir}/${paragraph_count}.docx" .
  )
  rm -rf "${work_dir}"
done
