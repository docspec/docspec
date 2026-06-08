# AGENTS.md

DocSpec is a streaming document conversion library in Rust. Documents are streams of typed events (StartHeading, Text, EndHeading, etc.) flowing from EventSource readers to EventSink writers. Readers and writers are fully decoupled. This architecture processes documents larger than available memory using constant memory regardless of file size.

## Read These First

- [MANIFESTO.md](MANIFESTO.md) — philosophy and values
- [ARCHITECTURE.md](ARCHITECTURE.md) — streaming pipeline and design
- [EVENTS.md](EVENTS.md) — event types and well-formedness rules
- [CODING_STANDARDS.md](CODING_STANDARDS.md) — code quality rules
- [TESTING.md](TESTING.md) — 98% new-work coverage requirement and test types
- [CONTRIBUTING.md](CONTRIBUTING.md) — branching, commits, PRs, semver
- [SECURITY.md](SECURITY.md) — error handling by context, resource limits

## Hard Rules

The rules below apply to **source code** unless marked otherwise. Test files (under `tests/` and `#[cfg(test)]` modules) may opt out of specific clippy strictness — see [Test Code Exceptions](#test-code-exceptions).

- **No unsafe code** — workspace forbids it entirely (applies everywhere, including tests)
- **No unwrap() or expect() in source code** — use Result and ? (tests may opt out)
- **No inline #[allow] in source code** — fix the code, not the warning (tests may opt out at crate level)
- **Never buffer full documents** — stream always
- **Fail fast** — return errors immediately
- **98% coverage floor for new and changed executable Rust lines in covered crates**
- **Exact-value test assertions** — no substring, type-only, or shape-only checks, see [TESTING.md](TESTING.md#exact-value-assertions)
- **Do not manually create or edit `CHANGELOG.md` files** — release-plz maintains them automatically from Conventional Commits. The root CHANGELOG.md is the only changelog; per-crate CHANGELOG.md files were removed in v1.5.0.

### Test Code Exceptions

Test files (`tests/**` integration tests and `#[cfg(test)]` modules) may use crate-level `#![allow(clippy::unwrap_used, clippy::expect_used, ...)]` to opt out of specific clippy lints that legitimately only apply to production code. Test setup, fixture loading, and assertions often need to panic on programmer error — banning unwrap there forces awkward boilerplate without making tests safer. Source code remains strictly enforced.

See [CODING_STANDARDS.md](CODING_STANDARDS.md) for full details.

## Before You Submit

- `cargo fmt` passes (applies to all `.rs` files, source and tests)
- `cargo clippy` passes with zero warnings — source code holds the strict line; test files may suppress specific lints via crate-level `#![allow(...)]`
- All tests pass, coverage maintained
- All public items have doc comments
- Commits follow conventional format: `type(scope): description`

See [CONTRIBUTING.md](CONTRIBUTING.md) for commit format and PR process.

## Core Pattern

Sources emit events in document order. Sinks consume them. Adapters transform between them. Events flow one at a time. Never accumulate. Never buffer.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the streaming pipeline design and [EVENTS.md](EVENTS.md) for event types.
