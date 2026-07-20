# Coding Standards

We do not suppress warnings. We fix the underlying problems. These standards exist because we have seen what happens when they are not followed: production crashes, silent data corruption, security vulnerabilities, code no one dares touch. The compiler is our first reviewer. It never gets tired. A warning that is ignored will become a bug.

For the exact lint configuration, see the workspace `Cargo.toml`. For the philosophy behind these rules, see [MANIFESTO.md](MANIFESTO.md).

---

## 1. The Underlying Philosophy

Code quality is about correctness, safety, and maintainability. We enforce standards that eliminate whole categories of bugs. Every rule exists because we have seen what happens when it is not followed. The compiler is our ally. We do not bypass it.

---

## 2. Safety: No Unsafe Code

`unsafe_code` is set to `"forbid"` in the workspace. No unsafe blocks. No exceptions.

Unsafe code opts out of Rust's ownership model and type system. It opens the door to memory corruption, data races, undefined behavior. If you think you need unsafe, the design needs to change. Use safe abstractions. The streaming architecture was designed specifically to avoid unsafe.

---

## 3. Documentation: Missing Docs Is Denied

`missing_docs` is set to `"deny"`. Every public function, struct, enum, trait, constant, and module must have a documentation comment.

Undocumented public API is incomplete API. Document the intent, not the implementation. "Returns the writer's current byte count" is good. "Increments self.count and returns it" is bad. Document panics, errors, safety invariants, and performance characteristics. Use examples.

---

## 4. Error Handling: Unwrap and Expect Are Banned in Source Code

`unwrap()` and `expect()` are banned in source code. The workspace sets `unwrap_used = "deny"` and `expect_used = "deny"` under `[workspace.lints.clippy]`. Every operation in source code that can fail returns a Result. Every error propagates with the `?` operator.

`unwrap()` is a panic waiting to happen. In a library, panics are unacceptable. The `?` operator makes propagation ergonomic. If you need a default, use `unwrap_or` or `unwrap_or_else`. Error types are descriptive and implement `std::error::Error` for interoperability.

**Test code exception**: Test files (under `tests/**` and `#[cfg(test)]` modules) may opt out via crate-level `#![allow(clippy::unwrap_used, clippy::expect_used)]`. Test setup, fixture parsing, and assertions frequently want to panic on programmer error — forcing `Result` propagation through every line of every test produces awkward boilerplate without making tests safer. The exception is scoped: it covers `unwrap_used` and `expect_used` (and a small number of related lints like `indexing_slicing`, `panic`), not the broader linting policy. Source code remains strictly enforced.

---

## 5. Linting: No Warning Suppressions in Source Code

No inline `#[allow]` attribute is permitted in source code. If clippy flags something in source code, we fix it — not the warning.

Warnings are signal. Suppressing them hides the signal. We run Clippy at the strictest settings: restriction, pedantic, correctness, and a curated set of nursery lints, all at deny level. If Clippy is genuinely wrong about a source-code lint (rare), we add a workspace-level exception in `Cargo.toml` with documented reasoning — never inline `#[allow]` in source files.

**Test code exception**: The workspace sets `clippy::allow_attributes = "allow"` precisely so that test files can use crate-level `#![allow(...)]` to opt out of specific lints that legitimately only apply to production code (most commonly `clippy::unwrap_used` and `clippy::expect_used`; sometimes `clippy::indexing_slicing` or `clippy::panic`). The test exception is for `#![allow(...)]` at the crate root of a test file — not for sprinkling inline `#[allow]` attributes throughout test bodies. Source code is held to the stricter standard by convention and by the per-lint denials that remain in effect everywhere (e.g. `unwrap_used = "deny"`).

---

## 6. Code Formatting: Consistency Over Style

`cargo fmt` is non-negotiable. All code is formatted before commit (enforced by pre-commit hooks).

Consistency eliminates style debates and makes diffs cleaner. If rustfmt produces unexpected output, the code structure is the problem, not the formatter. Refactor the code so the formatter is happy.

---

## 7. Naming Conventions

Rust has well-established naming conventions. We follow them without deviation:

- Types (structs, enums, traits): PascalCase
- Functions, methods, variables: snake_case
- Constants, statics: SCREAMING_SNAKE_CASE
- Modules, crates: snake_case (kebab-case for crate names)
- Lifetimes: short, single-letter or short descriptive ('a, 'doc, 'sink)

Names should describe what something IS or DOES, not how it works internally. `ConversionError`, not `InternalConversionResultWrapper`. Boolean names should read naturally in conditionals: `is_valid`, not `valid_flag`.

Identifier length: prefer descriptive names of four or more characters. One-to-three-character identifiers such as `hv`, `s`, `tmp`, `buf` obscure intent in non-trivial scopes — use `header_val`, `header_str`, `temp_buffer`, `output` instead. New crates enforce this with `clippy::min_ident_chars` (threshold 3) plus a crate-local `clippy.toml`. Legacy crates retain short idiomatic identifiers (`id`, RGB color channels `r`/`g`/`b`, standard imports like `use std::io`) until incrementally migrated.

---

## 8. Fail-Fast Error Propagation

Return errors immediately on failure. No partial recovery. If an operation fails, propagate the error to the caller.

Partial recovery creates ambiguity. A clear error is better than a silent partial success. Error propagation respects the caller's autonomy. The caller knows the context and whether to retry, abort, provide defaults, or escalate. Propagation is the default via the `?` operator.

---

## 9. The Review Standard

Code review is where standards are enforced. Reviewers check:

- Does this code do what it claims?
- Are there suppressions or workarounds?
- Is the documentation complete?
- Are errors handled correctly?
- Is coverage maintained?

Review is the last line of defense. Automated tools catch mechanical issues. Review catches design issues, logic errors, and deviations that tools cannot detect. Every PR is reviewed by at least one person other than the author. No self-merge.

---

## 10. Testing: Verification Over Trust

See [TESTING.md](TESTING.md) for our complete testing philosophy, coverage requirements, [exact-value assertion rules](TESTING.md#exact-value-assertions), and test type guidelines.

---

## 11. Dependencies: Earned, Not Assumed

Every new dependency requires written justification in the pull request description. What does it do? Why can we not do it ourselves? What is the cost?

Dependencies are liabilities. They update, break, have licenses, vulnerabilities, and transitive dependencies. Small code you wrote is better than large code you imported. When we do add a dependency, we pin it carefully and audit it periodically.

---

## 12. Type Safety: Leverage the Compiler

Use the type system to prevent bugs. Encode invariants in types. Use newtypes to distinguish between different kinds of strings. Use enums instead of booleans when there are more than two states.

The compiler can check types. It cannot check comments. When you encode an invariant in a type, the compiler enforces it. Use `NonZeroUsize` when zero is invalid. Use `Option` when a value might be absent. Use `Result` when an operation might fail.

---

## 13. Comments: Explain the Why

Comments should explain why code exists, not what it does. The code tells you what it does. Comments tell you why it does it. If the code is unclear, rewrite the code.

Use comments for: explaining why a non-obvious approach was chosen, documenting workarounds for external bugs, warning about subtle behavior, referencing specifications. Do not use comments for: restating what the code does, marking sections, leaving notes to yourself.

---

## 14. Module Organization

Modules should be small and focused. A module does one thing completely. It has a clear interface. Internal details are private.

Small modules are understandable. Clear interfaces reduce coupling. Use `mod.rs` to define the public interface. Re-export public items explicitly. Keep implementation details in submodules. If a file exceeds 300 lines, consider splitting it.

---

## Summary

These standards exist to produce correct, safe, maintainable code. They are enforced automatically where possible and by review where necessary. They are not suggestions. They are requirements.

Quality is not accidental. It is the result of discipline applied consistently over time.

---

## Further Reading

- **[MANIFESTO.md](MANIFESTO.md)** — The philosophy that drives these standards
- **[ARCHITECTURE.md](ARCHITECTURE.md)** — The streaming pipeline design
- **[CONTRIBUTING.md](CONTRIBUTING.md)** — The workflow that enforces these standards
