# `docspec-cli`

Command-line interface for DocSpec document conversion.

See the [main DocSpec repository](https://github.com/docspec/docspec) for documentation.

## Usage

```bash
docspec <COMMAND> [OPTIONS]
```

Commands:
- `convert` — Convert documents between formats
- `http` — Run the HTTP API server

### `convert` subcommand

```bash
docspec convert [OPTIONS] [INPUT]
```

#### Arguments

- `INPUT` — Input file (use `-` or omit for stdin)

#### Options

- `-o, --output <FILE>` — Output file (stdout if omitted)
- `-f, --from <FORMAT>` — Input format (auto-detected from extension if omitted). Valid values: `markdown`, `html`, `docx`
- `-t, --to <FORMAT>` — Output format (auto-detected from extension if omitted). Valid values: `blocknote`, `html`, `markdown`, `oxa`, `pandoc-native`
- `--color <WHEN>` — When to use colors: `auto`, `always`, `never` (default: `auto`)
- `-h, --help` — Print help
- `-V, --version` — Print version

### `http` subcommand

```bash
docspec http [OPTIONS]
```

Starts the HTTP API server. Listens on `127.0.0.1:3000` by default.

#### Options

- `--host <HOST>` — Address to bind the server to (default: `127.0.0.1`)
- `--port <PORT>` — Port to listen on. Use `0` for OS-assigned (default: `3000`)
- `-h, --help` — Print help

### Feature flags

`docspec-cli` ships with `http` enabled by default. For a slim install without the HTTP server stack:

```bash
cargo install docspec-cli --no-default-features
```

The resulting binary will only support `docspec convert`; running `docspec http` will print "unknown subcommand".

## Supported Input Formats

- `markdown` — Full Markdown support including headings, lists, tables, and inline formatting
- `html` — HTML input (see note below)
- `docx` — DOCX input (paragraphs and text only)

> **Note:** HTML and DOCX input currently preserve only paragraph text. Other HTML input
> elements and non-paragraph output events (headings, lists, tables, formatting tags, etc.)
> are silently dropped. DOCX input preserves only paragraphs and text; styles, tables, lists,
> images, headers/footers, and tracked changes are silently dropped. For fuller feature
> coverage, use Markdown input with BlockNote JSON output.

## Examples

Convert a Markdown file to BlockNote JSON:

```bash
docspec convert --from markdown --to blocknote input.md --output output.json
```

Convert an HTML file to BlockNote JSON (paragraphs only):

```bash
docspec convert --from html --to blocknote input.html --output output.json
```

Convert a DOCX file to BlockNote JSON (paragraphs only):

```bash
docspec --from docx --to blocknote input.docx --output output.json
```

Convert Markdown from stdin to BlockNote JSON on stdout:

```bash
echo "# Hello" | docspec convert --from markdown --to blocknote
```

Convert Markdown to HTML:

```bash
echo "Hello" | docspec convert --from markdown --to html
```

Convert Markdown to Pandoc native syntax:

```bash
echo "Hello" | docspec convert --from markdown --to pandoc-native
```

Convert Markdown to Markdown (round-trip, paragraphs and headings only):

```bash
echo "# Hello" | docspec convert --from markdown --to markdown
```

Start the HTTP API server on a custom port:

```bash
docspec http --port 8080
```

`--to oxa` selects the [oxa.dev](https://oxa.dev/) JSON writer in place of BlockNote. The `.json`
extension is ambiguous, so `--to oxa` must be explicit. HTML output is selected by `--to html`
or auto-detected from `.html` and `.htm` output paths. Pandoc native output is selected by
`--to pandoc-native` or auto-detected from `.native` output paths. Markdown output is selected
by `--to markdown` or auto-detected from `.md` output paths.
