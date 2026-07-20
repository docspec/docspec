# DocSpec

**Streaming document conversion that never buffers the whole document.**

Documents, as they flow. DocSpec reads a document as a stream of typed events and
writes it back out in another format — one event at a time, in constant memory, no
matter how large the file. Streaming is the whole point; the [Manifesto](MANIFESTO.md)
explains why.

**Funded by** [NLnet](https://nlnet.nl) through the [NGI0 Commons Fund](https://nlnet.nl/commonsfund/), and the Netherlands' [Ministry of the Interior and Kingdom Relations](https://www.rijksoverheid.nl/ministeries/ministerie-van-binnenlandse-zaken-en-koninkrijksrelaties).

<p>
  <a href="https://nlnet.nl"><img src="https://raw.githubusercontent.com/docspec/.github/main/assets/nlnet-banner.svg" alt="NLnet" height="40"></a>
  &nbsp;&nbsp;
  <a href="https://nlnet.nl/commonsfund/"><img src="https://raw.githubusercontent.com/docspec/.github/main/assets/ngi0-commons.svg" alt="NGI0 Commons Fund" height="40"></a>
  &nbsp;&nbsp;
  <a href="https://www.rijksoverheid.nl/ministeries/ministerie-van-binnenlandse-zaken-en-koninkrijksrelaties"><img src="https://raw.githubusercontent.com/docspec/.github/main/assets/minbzk.jpg" alt="Ministerie van Binnenlandse Zaken en Koninkrijksrelaties" height="40"></a>
</p>

**Used by** [DINUM — La Suite Numérique](https://lasuite.numerique.gouv.fr).

<p>
  <a href="https://lasuite.numerique.gouv.fr"><img src="https://raw.githubusercontent.com/docspec/.github/main/assets/dinum-gouv.svg" alt="Gouvernement" height="36"></a>
  &nbsp;&nbsp;
  <a href="https://lasuite.numerique.gouv.fr"><img src="https://raw.githubusercontent.com/docspec/.github/main/assets/lasuite.svg" alt="La Suite Numérique" height="36"></a>
</p>

## Getting started

Two ways in, depending on what you need.

### Convert a file — the CLI

Install with Cargo:

```sh
cargo install docspec-cli
```

Convert a document — the formats are inferred from the file extensions:

```sh
docspec convert report.docx report.md
```

Or pipe through stdin and name the formats explicitly:

```sh
echo '# Hello' | docspec convert --from markdown --to blocknote
```

### Run the API — Docker or the CLI

The `ghcr.io/docspec/api` image is the conversion server, ready to run with no Rust toolchain:

```sh
docker run --rm -p 3000:3000 ghcr.io/docspec/api
```

Installed the CLI instead? It ships the same server:

```sh
docspec http            # serves on 127.0.0.1:3000
```

Either way, send it a document — the `Content-Type` picks the reader, the `Accept` header picks the writer (BlockNote JSON by default):

```sh
curl -X POST http://localhost:3000/conversion \
     -H 'Content-Type: text/markdown' \
     -d '# Hello'
```

`GET /health` is the liveness check; [`docspec-http`](crates/docspec-http) has the full endpoint, header, and format reference.

## What it reads and writes

Readers and writers are fully decoupled: any reader can feed any writer, because the
event stream is the only contract between them.

| Direction  | Formats                                                     |
| ---------- | ----------------------------------------------------------- |
| **Reads**  | DOCX, HTML, Markdown                                         |
| **Writes** | HTML, Markdown, BlockNote JSON, oxa.dev JSON, Pandoc native  |

## Why DocSpec?

Most document converters load the whole file into memory, build a tree, and hope it fits. That works until someone uploads a 200 MB report — or until you need the same converter to run inside a browser tab. DocSpec exists to turn real-world documents into what modern **web editors** actually consume — BlockNote JSON, oxa.dev — without ever holding the whole document at once.

Streaming event by event, in constant memory, is the one decision everything else follows from:

- **It runs where your editor runs.** The same Rust compiles to a native server, a WebAssembly module in the browser, or an embedded target — one codebase, every surface, no separate front-end and back-end converters drifting apart.
- **Its memory stays flat.** A 1 KB note and a 500 MB document cost about the same to convert. Constant memory is the architecture, not a tuning flag.
- **It stays honest.** On corrupt input it stops and says so — no half-converted output quietly reaching your database.

DocSpec is the Rust successor to [NLdoc](https://gitlab.com/logius/nldoc), rebuilt for public-sector digital sovereignty: funded by [NLnet](https://nlnet.nl) and the Dutch Ministry of the Interior, and used in production by [La Suite](https://lasuite.numerique.gouv.fr) to import documents into its collaborative editor. The [Manifesto](MANIFESTO.md) is the long version of why.

## The crates

DocSpec is a Cargo workspace. Most users want the [`docspec`](crates/docspec) facade;
reach for the individual crates when you want the smallest possible dependency footprint.

**Core**

- [`docspec`](crates/docspec) — convenience facade; readers, writers, and core types behind feature flags
- [`docspec-core`](crates/docspec-core) — the event model, the `EventSource`/`EventSink` traits, and the streaming pipeline

**Readers**

- [`docspec-docx-reader`](crates/docspec-docx-reader) — DOCX
- [`docspec-html-reader`](crates/docspec-html-reader) — HTML
- [`docspec-markdown-reader`](crates/docspec-markdown-reader) — Markdown (CommonMark + GFM)

**Writers**

- [`docspec-html-writer`](crates/docspec-html-writer) — HTML
- [`docspec-markdown-writer`](crates/docspec-markdown-writer) — Markdown
- [`docspec-blocknote-writer`](crates/docspec-blocknote-writer) — BlockNote JSON
- [`docspec-oxa-writer`](crates/docspec-oxa-writer) — oxa.dev JSON
- [`docspec-pandoc-native-writer`](crates/docspec-pandoc-native-writer) — Pandoc native

**Primitives & tooling**

- [`docspec-json`](crates/docspec-json) — streaming JSON emission primitives for custom writers
- [`docspec-cli`](crates/docspec-cli) — the `docspec` command-line binary
- [`docspec-http`](crates/docspec-http) — the HTTP conversion server
- [`docspec-wasm`](crates/docspec-wasm) — WebAssembly bindings

## Documentation

- [Manifesto](MANIFESTO.md) — philosophy and values
- [Architecture](ARCHITECTURE.md) — the streaming pipeline, reader/writer contracts, and the event model
- [Coding Standards](CODING_STANDARDS.md) — the code style and the review bar
- [Contributing](CONTRIBUTING.md) — workflow, commits, PRs, semver
- [Bug Triage & Reporting](TRIAGE.md) — filing actionable bug reports and how we triage them
- [Testing](TESTING.md) — the coverage floor and test philosophy
- [Security](SECURITY.md) — error handling by context, resource limits
- [Agents](AGENTS.md) — guidance for AI agents working in this repo

## What we stand for

- **Memory conscious** — constant memory regardless of file size; every allocation earns its keep.
- **Streaming first** — events flow one at a time; nothing accumulates.
- **Fail fast** — on corruption we stop and say so: no partial output, no silent truncation.
- **No unsafe** — the workspace forbids `unsafe` entirely.
- **Proven** — a 98% coverage floor on new and changed executable Rust lines in covered crates; no `unwrap` or `expect` in source.

## Status

DocSpec is under active development. The architecture is stable and the event model is
defined; readers and writers land incrementally. We keep the public API honest about
what is supported today versus what a crate silently drops.

## Contributing

DocSpec is built in the open, and contributions are welcome — a new reader or writer, a bug fix, a sharper doc, a failing test that pins down an edge case. We hold a high bar (no `unsafe`, no `unwrap` in source, a 98% coverage floor on new and changed executable Rust lines in covered crates), but the bar is written down, not hidden. Start with [CONTRIBUTING.md](CONTRIBUTING.md) for the full workflow.

A few good entry points:

- **Found a bug?** [Bug Triage & Reporting](TRIAGE.md) shows what makes a report we can fix today. Security issues go [privately](SECURITY.md) — never a public issue.
- **Have an idea, or a design to talk through?** Open a [Discussion](https://github.com/orgs/docspec/discussions).
- **Ready to write code?** [Building and Running Locally](CONTRIBUTING.md#building-and-running-locally) gets you set up in a few commands.

Every contribution is attributed in the git history, and we treat review as collaboration, not gatekeeping.

## License

[MIT](LICENSE).

## Thanks

This work grew out of the mentoring and support of
[@virgile-dev](https://github.com/virgile-dev).
