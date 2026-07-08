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

`docspec-cli` ships with `http` and `posthog` enabled by default. Sentry compiles in via
`docspec-http`'s defaults. The telemetry integrations are runtime no-ops until their respective env vars are set —
see the [`docspec-http` README](https://docs.rs/docspec-http) for the full list.

For a slim install without the HTTP server stack (and therefore no telemetry):

```bash
cargo install docspec-cli --no-default-features
```

The resulting binary will only support `docspec convert`; running `docspec http` will print "unknown subcommand".

## Supported Input Formats

- `markdown` — Full Markdown support including headings, lists, tables, and inline formatting
- `html` — HTML input (see note below)
- `docx` — DOCX input including paragraphs, tables, ordered/unordered lists, hyperlinks, embedded images, line breaks, tabs, and run styles (bold, italic, underline, strikethrough, sub/superscript, color, highlight). Embedded images stream as base64 data URLs in BlockNote output. See [`docspec-docx-reader`](https://docs.rs/docspec-docx-reader) for the authoritative list of supported and out-of-scope OOXML elements.

> **Note:** HTML input is paragraph-only — the HTML reader currently parses `<p>` elements only,
> and other HTML elements are silently dropped. DOCX input is considerably richer (see above);
> known DOCX elements outside scope (headings via `<w:pStyle>`, vertical cell merges, VML images,
> comments, footnotes, headers/footers, document metadata) are silently dropped per the
> [DOCX reader denylist](https://docs.rs/docspec-docx-reader). For HTML, use Markdown input or
> DOCX input with BlockNote JSON output for fuller feature coverage.

## Examples

Convert a Markdown file to BlockNote JSON:

```bash
docspec convert --from markdown --to blocknote input.md --output output.json
```

Convert an HTML file to BlockNote JSON (paragraphs only):

```bash
docspec convert --from html --to blocknote input.html --output output.json
```

Convert a DOCX file to BlockNote JSON (preserves paragraphs, tables, lists, hyperlinks, images, and run styles):

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
