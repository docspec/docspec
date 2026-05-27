# docspec-cli

Command-line interface for DocSpec document conversion.

See the [main DocSpec repository](https://github.com/docspec/docspec) for documentation.

## Usage

```bash
docspec [OPTIONS] [INPUT]
```

### Arguments

- `INPUT` — Input file (use `-` or omit for stdin)

### Options

- `-o, --output <FILE>` — Output file (stdout if omitted)
- `-f, --from <FORMAT>` — Input format (auto-detected from extension if omitted)
- `-t, --to <FORMAT>` — Output format (auto-detected from extension if omitted)
- `--color <WHEN>` — When to use colors: `auto`, `always`, `never` (default: `auto`)
- `-h, --help` — Print help
- `-V, --version` — Print version

## Memory Characteristics

**File input** uses `memmap2` to memory-map the file. The kernel pages the file on demand — heap usage is proportional to the working set (parser overhead), not the file size. A 100 MB file uses the same heap as a 1 MB file.

**Stdin input** reads to a `String` (documented limitation: pipes cannot be memory-mapped). For large stdin inputs, consider writing to a temporary file first.

## Unsafe Code

`docspec-cli` contains exactly one `unsafe` block: the `memmap2::Mmap::map` call in `src/input.rs`. This is documented in `CODING_STANDARDS.md` Section 2. All library crates (`docspec-core`, `docspec-json`, `docspec-markdown-reader`, `docspec-blocknote-writer`, `docspec-wasm`) contain zero unsafe code.
