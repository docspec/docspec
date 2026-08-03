# Contributing to DocSpec

We are not relaxed. We are not permissive. We have standards, and we enforce them — not as gatekeeping, but as stewardship. Read the [Manifesto](MANIFESTO.md) before diving into code. It explains what we stand for: memory efficiency, streaming design, and strict quality above convenience. Then come back here for the workflow.

## Getting Started

### Prerequisites

- Rust stable toolchain (latest stable version)
- Git with your user name and email configured
- [Git LFS](https://git-lfs.com/) for large binary test fixtures — run `git lfs install` once per machine after installing; run `git lfs pull` after clone to fetch the `.docx` test fixtures
- [pre-commit](https://pre-commit.com/) for running pre-commit hooks
- [taplo](https://taplo.tamasfe.dev/) for TOML formatting (`cargo install taplo-cli`)
- [`just`](https://github.com/casey/just) for the build and test commands below (`cargo install just`)

Clone the repository:
```bash
git clone https://github.com/docspec/docspec.git
cd docspec
pre-commit install --hook-type pre-commit
pre-commit install --hook-type commit-msg
pre-commit install --hook-type pre-push
```

The pre-commit stage (runs on every commit) enforces formatting, linting, and hygiene checks. The pre-push stage (runs before push) runs the full build, test suite, and documentation build.

### Hook Bypass Policy

Use `git commit --no-verify` or `git push --no-verify` only when:
- Fixing a broken hook configuration (the hook itself is the problem)
- Work-in-progress commits on a personal branch that will be squashed before PR

Never bypass hooks on commits intended for pull request review. CI will catch what hooks miss, but hooks exist to give you fast local feedback.

## Building and Running Locally

The `justfile` is the front door. Running `just` with no arguments runs the whole local gate — format, lint, test, doc, build — the same checks the hooks and CI enforce:

```bash
just              # fmt · clippy · test · doc · build
```

Reach for a single recipe when you want one step:

```bash
just build        # cargo build --workspace
just test         # cargo test --workspace
just clippy       # cargo clippy --workspace --all-targets -- -D warnings
just fmt          # cargo fmt --all
just doc          # cargo doc --workspace --no-deps, warnings denied
just coverage     # llvm-cov across covered crates → lcov.info
```

Every recipe is a thin wrapper over the Cargo command shown beside it, so raw `cargo` works just as well. `just --list` prints the full set.

### Running the CLI from source

The binary is `docspec`. Run it through Cargo without installing anything:

```bash
cargo run -p docspec-cli -- convert report.docx --output report.md
cargo run -p docspec-cli -- convert --from markdown --to blocknote input.md
cargo run -p docspec-cli -- --help
```

### Running the HTTP server from source

```bash
cargo run -p docspec-cli -- http                          # binds 127.0.0.1:3000
cargo run -p docspec-cli -- http --host 0.0.0.0 --port 8080
```

`GET /health` reports liveness, `POST /conversion` streams a conversion, and `GET /metrics` exposes Prometheus metrics. Sentry and PostHog are compiled in but stay dormant unless `DOCSPEC_SENTRY_DSN` or `DOCSPEC_POSTHOG_API_KEY` is set — nothing leaves your machine by default.

### Feature flags

`just build` and `just test` run `cargo build/test --workspace`, which builds each crate with its own default feature set — so every reader and writer *crate* is compiled and tested (each has meaningful defaults), but non-default facade feature combinations are only checked when you enable them explicitly. Flags matter when you depend on the [`docspec`](crates/docspec) facade and want a smaller footprint:

- Facade defaults are `markdown`, `blocknote`, and `pandoc-native`.
- DOCX reading is opt-in — enable the `docx` feature, or `full` for everything.
- The CLI bundles the HTTP server by default; `cargo install docspec-cli --no-default-features` builds a convert-only binary.

### WebAssembly

`docspec-wasm` builds with [`wasm-pack`](https://rustwasm.github.io/wasm-pack/) (`cargo install wasm-pack`) against the `wasm32-unknown-unknown` target:

```bash
just wasm          # wasm-pack build --dev --target web crates/docspec-wasm
just wasm-release  # the optimized build
```

The output bundle lands in `crates/docspec-wasm/pkg/`.

### Docker

The `Dockerfile` at the repo root is the image published as `ghcr.io/docspec/api` — a static `docspec` binary running `http` on port 3000:

```bash
docker build -t docspec-api .
docker run --rm -p 3000:3000 docspec-api
```

## Branching Strategy

We follow GitHub Flow:

1. Create a feature branch from `main`
2. Make your changes with clean, focused commits
3. Push your branch and open a pull request to `main`
4. Address review feedback
5. A reviewer merges when approved

### Branch Naming

Use descriptive branch names:
```
feat/add-odt-writer
fix/docx-image-handling
docs/update-api-examples
refactor/event-pipeline
test/html-roundtrip-coverage
```

## Commit Messages

Every commit must follow the [Conventional Commits](https://www.conventionalcommits.org/) specification (enforced by pre-commit hook).

### Format

```
<type>(<scope>): <description>

[optional body]
[optional footer(s)]
```

### Types

- **feat**: New feature or capability
- **fix**: Bug fix
- **docs**: Documentation only changes
- **refactor**: Code change that neither fixes a bug nor adds a feature
- **test**: Adding or correcting tests
- **chore**: Maintenance tasks, dependency updates, tooling changes

### Examples

```
feat(http): add streaming response support
```

```
fix(docx): handle corrupted image entries

Previously, corrupted image entries in DOCX files caused a panic.
Now we surface a proper error and abort gracefully.
```

```
chore(deps): update zip to 2.2.1

Security advisory GHSA-xxxx-xxxx-xxxx affects zip < 2.2.0.
```

### Guidelines

- Use imperative mood: "add" not "added", "fix" not "fixed"
- Do not capitalize the first letter of the description
- No period at the end of the description
- Keep the first line under 72 characters
- Use the body to explain WHY the change was made, not WHAT was changed

## Pull Requests

### Opening a Pull Request

- Push your branch to the repository
- Open a pull request against `main`
- Use conventional commit format for the PR title
- Fill out the PR description template completely
- Link related issues: `Fixes #123`, `Closes #456`, `Relates to #789`

### Review Requirements

Every pull request requires at least one reviewer approval before merging. Reviewers verify:
- All CI checks pass (tests, lint, format)
- New and changed executable Rust lines in covered crates meet the 98% coverage floor
- New code has tests covering the changes
- Public APIs are documented
- Error handling in source code is thorough (no unwrap, no expect; test files may opt out via crate-level `#![allow(clippy::unwrap_used, clippy::expect_used)]`)
- Memory usage is reasonable and documented if changed
- The change fits the streaming-first architecture

### Review Process

1. Author opens PR
2. Reviewer requests changes or approves
3. Author addresses feedback (amend and force push, or add commits)
4. Reviewer approves or requests additional changes
5. Once approved, the reviewer (not the author) merges

Review is collaboration, not gatekeeping. Ask questions. Push back when needed.

## Code Standards

See [CODING_STANDARDS.md](CODING_STANDARDS.md) for complete rules. The essentials: no unsafe anywhere; in source code, no `unwrap`/`expect` and no inline `#[allow]`; all public items documented; 98% coverage for new and changed executable Rust lines in covered crates; `cargo fmt` before every commit. Test files (`tests/**` and `#[cfg(test)]` modules) may opt out of `unwrap_used` / `expect_used` (and related test-only lints) via crate-level `#![allow(...)]`; source code may not.

## Adding Dependencies

Every new dependency requires written justification in the pull request description. Dependencies are liabilities, not conveniences. Your justification must explain:
- What problem the dependency solves
- Why we cannot solve it ourselves in reasonable code
- The transitive dependency count
- The maintenance status
- License compatibility

Example:

> Adding `memchr` for fast byte search within buffers.
> Problem: We need to find byte sequences in streaming buffers efficiently.
> Self-implementation: Possible but `memchr` uses SIMD and is heavily optimized.
> Transitive deps: 0 (no dependencies)
> Maintenance: Actively maintained by the `regex` crate author
> License: MIT + Unlicense (compatible)

## Versioning

DocSpec uses a single ecosystem version across all 12 crates. The version lives in `[workspace.package].version` in the root `Cargo.toml`, and every crate inherits it via `version.workspace = true`. Internal crate-to-crate dependencies are declared in `[workspace.dependencies]` and track the workspace version exactly.

SemVer applies at the ecosystem level:

- **MAJOR**: Any breaking change in any crate bumps the ecosystem major version
- **MINOR**: New features, backwards compatible
- **PATCH**: Bug fixes, backwards compatible

Ecosystem coherence takes priority over per-crate precision. A breaking change in one crate bumps the major version for the entire ecosystem, even for users who don't use that crate. This keeps the compatibility story simple: one version number means one coherent set of crates.

### Breaking Changes

Breaking changes ALWAYS bump the major version. No exceptions. Breaking changes include:
- Removing or renaming public functions, types, or modules
- Changing function signatures
- Changing behavior of existing functions
- Removing support for features or formats
- Changing error types or their behavior

After 1.0, breaking changes require strong justification. When in doubt, bump major.

The unified ecosystem version policy started at v1.5.0. Before that, crates used independent per-crate versioning. See [RELEASING.md §Historical Notes](RELEASING.md#historical-notes) for context on the old tags.

## Releases

DocSpec uses [release-plz](https://release-plz.dev) to automate version bumps, changelog updates, and publishing. release-plz reads Conventional Commits from `main`, opens a release PR, and on merge: tags `vX.Y.Z`, publishes all publishable crates via Trusted Publishing (OIDC), and creates a GitHub Release.

For the full maintainer runbook (Trusted Publishing setup, recovery procedures, manual operations, supply chain verification), see [RELEASING.md](RELEASING.md).

### Release Types

- `feat` commits trigger a minor version bump
- `fix` commits trigger a patch version bump
- Commits with `BREAKING CHANGE` in the footer trigger a major version bump

### Crates.io Publishing

Crates.io publishes use Trusted Publishing (OIDC); no stored API token is required. See [RELEASING.md](RELEASING.md) for setup instructions and the publish order.

Before merging a release PR, maintainers review the generated version bump, `CHANGELOG.md` diff, and publish plan as described in [RELEASING.md](RELEASING.md).

### Writing Good Commit Messages for Changelog

Your commit messages become the changelog. Write them for users, not for other maintainers.

Bad: `fix(parser): handle edge case in loop`

Better: `fix(docx): prevent panic on documents with missing content types`

## Questions and Support

- Open an issue for bug reports or feature requests — see [Bug Triage & Reporting](TRIAGE.md) for what makes a report actionable and how we triage
- Open an issue for usage or architecture questions too — GitHub Discussions is not currently enabled for this repository
- Read existing documentation before asking

## Attribution

Contributions are attributed in the git history. Your commits are your contribution record. By contributing, you agree that your contributions will be licensed under the same license as the project.
