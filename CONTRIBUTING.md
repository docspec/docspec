# Contributing to DocSpec

Thank you for considering a contribution. DocSpec is a memory-conscious streaming document conversion library with strict quality standards.

## Getting Started

### Prerequisites

- Rust stable toolchain (latest stable version)
- Git with your user name and email configured
- [pre-commit](https://pre-commit.com/) for running pre-commit hooks
- [taplo](https://taplo.tamasfe.dev/) for TOML formatting (`cargo install taplo-cli`)
- [typos](https://github.com/crate-ci/typos) for spell checking (`cargo install typos-cli`)

Clone the repository:
```bash
git clone https://github.com/docspec/docspec.git
cd docspec
pre-commit install --hook-type pre-commit
pre-commit install --hook-type commit-msg
pre-commit install --hook-type pre-push
```

The pre-commit hooks enforce formatting, linting, spell checking, and conventional commit message format; the pre-push hooks verify the full build, test suite, and documentation. Before diving into code, read the [Manifesto](MANIFESTO.md) to understand our philosophy: memory efficiency, streaming design, and strict quality above convenience.

The pre-commit stage (runs on every commit) enforces formatting, linting, and hygiene checks. The pre-push stage (runs before push) runs the full build, test suite, and documentation build.

### Hook Bypass Policy

Use `git commit --no-verify` or `git push --no-verify` only when:
- Fixing a broken hook configuration (the hook itself is the problem)
- Work-in-progress commits on a personal branch that will be squashed before PR

Never bypass hooks on commits intended for pull request review. CI will catch what hooks miss, but hooks exist to give you fast local feedback.

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

- Open an issue for bug reports or feature requests
- Use discussions for questions about usage or architecture
- Read existing documentation before asking

## Attribution

Contributions are attributed in the git history. Your commits are your contribution record. By contributing, you agree that your contributions will be licensed under the same license as the project.

---

Thank you for helping make DocSpec better.
