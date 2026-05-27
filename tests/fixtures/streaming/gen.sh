#!/usr/bin/env bash
# gen.sh — Generate synthetic fixture files for streaming memory tests.
#
# Produces {1,10,100}mb.{md,bin} in this directory.
# Files are git-ignored; regenerate as needed before running memory tests.
#
# Usage:
#   bash tests/fixtures/streaming/gen.sh

set -e

cd "$(dirname "$0")"

for N in 1 10 100; do
    TARGET=$((N * 1024 * 1024))

    # --- Binary file (byte-exact via dd) ---
    echo "Generating ${N}mb.bin (${TARGET} bytes)..."
    dd if=/dev/urandom bs=1024 count=$((N * 1024)) of="${N}mb.bin" 2>/dev/null

    # --- Markdown file (pseudo-content, byte-exact via head -c) ---
    echo "Generating ${N}mb.md (${TARGET} bytes)..."
    # Generate ~2x the target as base64 text (base64 expands ~33%, so urandom
    # input of TARGET bytes → ~1.33*TARGET base64 chars; we need TARGET chars
    # so we read 2*TARGET urandom bytes to be safe), then truncate exactly.
    head -c "$((TARGET * 2))" /dev/urandom \
        | base64 -w 80 \
        | awk 'NR % 10 == 0 { print; print "" } NR % 10 != 0 { print }' \
        | head -c "$TARGET" > "${N}mb.md"
done

echo ""
echo "Generated files:"
for N in 1 10 100; do
    for ext in md bin; do
        SIZE=$(stat -c%s "${N}mb.${ext}" 2>/dev/null || stat -f%z "${N}mb.${ext}")
        TARGET=$((N * 1024 * 1024))
        echo "  ${N}mb.${ext}: ${SIZE} bytes (target: ${TARGET})"
    done
done
