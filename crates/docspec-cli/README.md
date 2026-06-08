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
- `-f, --from <FORMAT>` — Input format (auto-detected from extension if omitted). Valid values: `markdown`, `html`
- `-t, --to <FORMAT>` — Output format (auto-detected from extension if omitted). Valid values: `blocknote`, `html`, `oxa`, `pandoc-native`
- `--color <WHEN>` — When to use colors: `auto`, `always`, `never` (default: `auto`)
- `-h, --help` — Print help
- `-V, --version` — Print version

## Supported Input Formats

- `markdown` — Full Markdown support including headings, lists, tables, and inline formatting
- `html` — HTML input (see note below)

> **Note:** HTML input and output currently preserve only paragraph text. Other HTML input
> elements and non-paragraph output events (headings, lists, tables, formatting tags, etc.)
> are silently dropped. For fuller feature coverage, use Markdown input with BlockNote JSON
> output.

## Examples

Convert a Markdown file to BlockNote JSON:

```bash
docspec --from markdown --to blocknote input.md --output output.json
```

Convert an HTML file to BlockNote JSON (paragraphs only):

```bash
docspec --from html --to blocknote input.html --output output.json
```

Convert Markdown from stdin to BlockNote JSON on stdout:

```bash
echo "# Hello" | docspec --from markdown --to blocknote
```

Convert Markdown to HTML:

```bash
echo "Hello" | docspec --from markdown --to html
```

Convert Markdown to Pandoc native syntax:

```bash
echo "Hello" | docspec --from markdown --to pandoc-native
```

`--to oxa` selects the [oxa.dev](https://oxa.dev/) JSON writer in place of BlockNote. The `.json`
extension is ambiguous, so `--to oxa` must be explicit. HTML output is selected by `--to html`
or auto-detected from `.html` and `.htm` output paths. Pandoc native output is selected by
`--to pandoc-native` or auto-detected from `.native` output paths.
