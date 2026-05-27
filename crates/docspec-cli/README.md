# docspec-cli

Command-line interface for [DocSpec](https://github.com/docspec/docspec) document conversion.

The CLI uses Pandoc-style subcommands:

```bash
docspec [GLOBAL OPTIONS] <COMMAND>
```

## Global Options

- `--color <WHEN>` — Color output: `auto`, `always`, `never` (default: `auto`)
- `-h, --help` — Print help
- `-V, --version` — Print version

## Convert Documents

```bash
docspec convert [OPTIONS] [INPUT]
```

`INPUT` is an input file. Use `-` or omit it to read from stdin.

Options:

- `-o, --output <FILE>` — Output file (stdout if omitted)
- `-f, --from <FORMAT>` — Input format (auto-detected from extension if omitted)
- `-t, --to <FORMAT>` — Output format (auto-detected from extension if omitted)
- `--list-input-formats` — List supported input formats and exit
- `--list-output-formats` — List supported output formats and exit
- `--verbose` — Print conversion completion message

Example:

```bash
docspec convert --from markdown --to blocknote input.md -o out.json
```

## Start the HTTP Server

```bash
docspec http [OPTIONS]
```

Options:

- `--host <HOST>` — Host address to bind (default: `127.0.0.1`)
- `--port <PORT>` — Port to listen on (default: `3000`)
- `--log-format <FORMAT>` — `pretty` or `json` (default: `pretty`)
- `--log-level <LEVEL>` — `trace`, `debug`, `info`, `warn`, `error` (default: `info`)

## Breaking Change

The root conversion command moved under `convert`. Migrate old invocations like
`docspec input.md -o out.json` to `docspec convert input.md -o out.json`.
