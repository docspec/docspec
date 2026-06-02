# `docspec-cli`

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
