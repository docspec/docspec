# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- New `docspec-http` crate: HTTP API server exposing the DocSpec streaming pipeline over HTTP using Axum 0.8 (issue #15, NLnet-funded milestone)
- `POST /convert` endpoint: accepts `text/markdown`, returns `application/vnd.docspec.blocknote+json` as a streaming response
- `GET /health` endpoint: returns `204 No Content` for health checks
- RFC 7807 problem+json error responses with `application/problem+json` Content-Type and `https://docspec.dev/errors/{code}` type URIs
- `docspec http` subcommand: starts the HTTP API server with `--host`, `--port`, `--log-level`, `--log-format` flags
- `docspec convert` subcommand: Pandoc-style document conversion with `-f/--from`, `-t/--to`, `-o/--output`, `--list-input-formats`, `--list-output-formats`, `--verbose` flags
- Structured tracing/observability via `tracing` + `tower-http::TraceLayer` with `x-request-id` header on all responses
- Graceful shutdown on `SIGINT`/`SIGTERM` (Unix) and `Ctrl-C` (all platforms)

### Changed

- **BREAKING**: CLI restructured to use subcommands. `docspec input.md -o out.json` is now `docspec convert input.md -o out.json`. All existing flags (`-f`, `-t`, `-o`, `--color`) are preserved under the `convert` subcommand, with `--color` remaining a top-level global flag.

### Migration Guide

Replace all invocations of the flat CLI with the `convert` subcommand:

```bash
# Before
docspec input.md -o output.json
docspec -f markdown -t blocknote < input.md

# After
docspec convert input.md -o output.json
docspec convert -f markdown -t blocknote < input.md
```
