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
- Test coverage remains at or above 99.5%
- New code has tests covering the changes
- Public APIs are documented
- Error handling is thorough (no unwrap, no expect)
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

See [CODING_STANDARDS.md](CODING_STANDARDS.md) for complete rules. The essentials: no unsafe, no unwrap/expect, no #[allow], all public items documented, 99.5% test coverage, `cargo fmt` before every commit.

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

DocSpec follows strict Semantic Versioning (semver):

- **MAJOR**: Breaking changes (API changes, behavior changes)
- **MINOR**: New features, backwards compatible
- **PATCH**: Bug fixes, backwards compatible

### Breaking Changes

Breaking changes ALWAYS bump the major version. No exceptions. Breaking changes include:
- Removing or renaming public functions, types, or modules
- Changing function signatures
- Changing behavior of existing functions
- Removing support for features or formats
- Changing error types or their behavior

During 0.x releases, breaking changes are expected and documented. After 1.0, breaking changes require strong justification. When in doubt, bump major.

## Releases

DocSpec uses [release-please](https://github.com/googleapis/release-please) to generate changelogs automatically from conventional commits. Do not edit the changelog manually—it is a function of commit history.

### How It Works

1. Commits land on main with conventional commit format
2. Release Please opens a release PR when enough changes accumulate
3. The release PR contains the proposed version bump and generated changelog
4. A maintainer merges the release PR
5. A new release is tagged and published automatically

### Release Types

- `feat` commits trigger a minor version bump
- `fix` commits trigger a patch version bump
- Commits with `BREAKING CHANGE` in the body trigger a major version bump

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
