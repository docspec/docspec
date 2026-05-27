# Streaming Test Fixtures

Synthetic large files for memory slope tests. Generated on demand; not committed to git.

## Usage

```bash
bash tests/fixtures/streaming/gen.sh
```

Run from any directory — the script changes to its own directory automatically.

## Files Generated

| File | Size | Format |
|------|------|--------|
| `1mb.md` | 1 MB | Pseudo-markdown (base64 text, paragraph breaks) |
| `1mb.bin` | 1 MB | Random binary |
| `10mb.md` | 10 MB | Pseudo-markdown |
| `10mb.bin` | 10 MB | Random binary |
| `100mb.md` | 100 MB | Pseudo-markdown |
| `100mb.bin` | 100 MB | Random binary |

Each file is within ±1 KB of its target size (`N * 1024 * 1024` bytes).

## Why Git-Ignored

These files are large (111 MB total) and fully reproducible. The `.gitignore` excludes `*.md` and `*.bin` so they are never accidentally committed.

## Used By

- **BUG-1 (T13)**: Memory slope test for markdown parsing — verifies heap usage grows O(1) not O(n) as input size scales from 1 MB → 10 MB → 100 MB.
- **BUG-3 (T10)**: Memory slope test for binary asset passthrough — same O(1) verification for binary streams.

Tests call `gen.sh` themselves if fixtures are absent; no manual pre-generation required.
