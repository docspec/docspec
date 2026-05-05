# AGENTS.md

DocSpec is a streaming document conversion library in Rust. Documents are streams of typed events (StartHeading, Text, EndHeading, etc.) flowing from EventSource readers to EventSink writers. Readers and writers are fully decoupled. This architecture processes documents larger than available memory using constant memory regardless of file size.

## Read These First

- [MANIFESTO.md](MANIFESTO.md) — philosophy and values
- [ARCHITECTURE.md](ARCHITECTURE.md) — streaming pipeline and design
- [EVENTS.md](EVENTS.md) — event types and well-formedness rules
- [CODING_STANDARDS.md](CODING_STANDARDS.md) — code quality rules
- [TESTING.md](TESTING.md) — 98% coverage requirement and test types
- [CONTRIBUTING.md](CONTRIBUTING.md) — branching, commits, PRs, semver
- [SECURITY.md](SECURITY.md) — error handling by context, resource limits

## Hard Rules

- **No unsafe code** — workspace forbids it entirely
- **No unwrap() or expect()** — use Result and ?
- **No inline #[allow]** — fix the code, not the warning
- **Never buffer full documents** — stream always
- **Fail fast** — return errors immediately
- **98% test coverage floor**

See [CODING_STANDARDS.md](CODING_STANDARDS.md) for full details.

## Before You Submit

- `cargo fmt` and `cargo clippy` pass with zero warnings
- All tests pass, coverage maintained
- All public items have doc comments
- Commits follow conventional format: `type(scope): description`

See [CONTRIBUTING.md](CONTRIBUTING.md) for commit format and PR process.

## Core Pattern

Sources emit events in document order. Sinks consume them. Adapters transform between them. Events flow one at a time. Never accumulate. Never buffer.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the streaming pipeline design and [EVENTS.md](EVENTS.md) for event types.
