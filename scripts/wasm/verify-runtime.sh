#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
package_root="${1:-${repository_root}/target/wasm-pack}"
playwright_root="${PLAYWRIGHT_ROOT:-${repository_root}/target/wasm-test-tools}"

"${repository_root}/scripts/wasm/generate-memory-fixtures.sh"
node "${repository_root}/scripts/wasm/runtime-node.cjs" "${package_root}"
NODE_PATH="${playwright_root}/node_modules" \
  node "${repository_root}/scripts/wasm/runtime-browser.cjs" "${package_root}"
NODE_PATH="${playwright_root}/node_modules" \
  node "${repository_root}/scripts/wasm/runtime-example-browser.cjs" "${package_root}"
