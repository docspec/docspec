# docspec-cli

**Command-line interface for DocSpec document conversion**

One binary, every format DocSpec speaks. `docspec-cli` wraps the full DocSpec conversion pipeline behind two subcommands: `convert` turns a file (or stdin) from one format into another, and `http` starts the HTTP API server. Streaming, like the rest of DocSpec. (See the [Manifesto](https://github.com/docspec/docspec/blob/main/MANIFESTO.md) for why.)

## Install it

```bash
cargo install docspec-cli
```

For a slim install without the HTTP server stack (and therefore no telemetry):

```bash
cargo install docspec-cli --no-default-features
```

The resulting binary supports only `docspec convert`; running `docspec http` will print "unknown subcommand".

## Convert a document

```bash
docspec [OPTIONS] <COMMAND>
```

Top-level options:
- `-h, --help` — Print help
- `-V, --version` — Print version

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

## Supported input formats

- `markdown` — Full Markdown support including headings, lists, tables, and inline formatting
- `html` — HTML input (see note below)
- `docx` — DOCX input including paragraphs, tables, ordered/unordered lists, hyperlinks, embedded images, line breaks, tabs, and run styles (bold, italic, underline, strikethrough, sub/superscript, color, highlight). Embedded images stream as base64 data URLs in BlockNote output. See [`docspec-docx-reader`](https://docs.rs/docspec-docx-reader) for the authoritative list of supported and out-of-scope OOXML elements.

> **HTML note:** HTML input is paragraph-only. The reader recognizes `<p>` elements,
> preserves text inside inline elements within those paragraphs, and silently drops all
> other structure.
>
> **DOCX note:** DOCX input supports paragraphs, style-derived headings, block quotes,
> preformatted blocks, tables, lists, hyperlinks, DrawingML and VML images, and run styles.
> Known losses include vertical cell merges, comments, footnotes, headers and footers,
> document metadata, tracked deletions, and field-code hyperlinks.

## Supported output formats

Only `blocknote` is production-ready. **Every other output format is experimental and will
silently drop structure it cannot express** — the conversion succeeds and exits `0`, but
content is missing from the output.

- `blocknote` — **Supported.** Headings, lists, tables, links, images, styles, block quotes, code
- `pandoc-native` — *Experimental.* Paragraphs, headings, text styles and code blocks; lists, tables, links and images are dropped
- `markdown` — *Experimental.* Paragraphs and headings only, text only
- `html` — *Experimental.* Paragraphs and text only; headings, lists, tables, links and images are dropped
- `oxa` — *Experimental.* Paragraphs and text only; headings, lists, tables, links and images are dropped

> **Worked example.** `echo '# Foo' | docspec convert --from markdown --to html` prints
> `<html><body></body></html>` — the heading is dropped, and its text with it, because the
> HTML writer only emits text inside a paragraph.

See the [format matrix](https://github.com/docspec/docspec/blob/main/README.md#what-it-reads-and-writes)
for the canonical status of every format.

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
docspec convert --from docx --to blocknote input.docx --output output.json
```

Convert Markdown from stdin to BlockNote JSON on stdout:

```bash
echo "# Hello" | docspec convert --from markdown --to blocknote
```

Convert Markdown to HTML (experimental — paragraphs and text only):

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

## Exit status

- `0` — command completed successfully
- `1` — conversion, I/O, or server runtime failure
- `2` — command-line usage or argument parsing error

## Related

- [Architecture](https://github.com/docspec/docspec/blob/main/ARCHITECTURE.md) — the streaming pipeline and event model
- [DocSpec repository](https://github.com/docspec/docspec)
