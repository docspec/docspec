# Coding Standards

This document explains why we enforce the standards we do. For the exact rules, see the workspace `Cargo.toml`. For the philosophy behind them, see `MANIFESTO.md`.

---

## 1. The Underlying Philosophy

Code quality is about correctness, safety, and maintainability. We enforce standards that eliminate whole categories of bugs. We do not suppress warnings; we fix the underlying problems.

Every rule exists because we have seen what happens when it is not followed: production crashes, silent data corruption, security vulnerabilities, code no one dares touch. The compiler is our first reviewer. It never gets tired. A warning that is ignored will become a bug.

---

## 2. Safety: No Unsafe Code

`unsafe_code` is set to `"deny"` at workspace level. No unsafe blocks. No exceptions in library crates.

Unsafe code opts out of Rust's ownership model and type system. It opens the door to memory corruption, data races, undefined behavior. If you think you need unsafe, the design needs to change. Use safe abstractions. The streaming architecture was designed specifically to avoid unsafe.

**Exception**: `docspec-cli` may use exactly ONE `#[allow(unsafe_code)]` attribute on the specific function or block calling `memmap2::Mmap::map` (a kernel-bridging operation that Rust convention treats as unsafe). The attribute MUST be accompanied by a `// SAFETY:` comment explaining the invariants maintained. Library crates (`docspec-core`, `docspec-json`, `docspec-markdown-reader`, `docspec-blocknote-writer`, `docspec-wasm`) remain 100% safe — verifiable via:

```sh
grep -rE '#\[allow\(unsafe_code' crates/docspec-{core,json,markdown-reader,blocknote-writer,wasm}/src
```

The above command must return zero matches.

---

## 3. Documentation: Missing Docs Is Denied

`missing_docs` is set to `"deny"`. Every public function, struct, enum, trait, constant, and module must have a documentation comment.

Undocumented public API is incomplete API. Document the intent, not the implementation. "Returns the writer's current byte count" is good. "Increments self.count and returns it" is bad. Document panics, errors, safety invariants, and performance characteristics. Use examples.

---

## 4. Error Handling: Unwrap and Expect Are Banned

`unwrap()` and `expect()` are banned across the codebase. Every operation that can fail returns a Result. Every error propagates with the `?` operator.

`unwrap()` is a panic waiting to happen. In a library, panics are unacceptable. The `?` operator makes propagation ergonomic. If you need a default, use `unwrap_or` or `unwrap_or_else`. Error types are descriptive and implement `std::error::Error` for interoperability.

---

## 5. Linting: No Warning Suppressions

`allow_attributes` is set to `"deny"`. No `#[allow]` attribute is permitted in source code. If clippy flags something, we fix it.

Warnings are signal. Suppressing them hides the signal. We run Clippy at the strictest settings: restriction, pedantic, and correctness groups, all at deny level. If Clippy is genuinely wrong (rare), we add a workspace-level exception in `Cargo.toml` with documented reasoning — never inline `#[allow]`.

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

See [TESTING.md](TESTING.md) for our complete testing philosophy, coverage requirements, and test type guidelines.

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

- **MANIFESTO.md** — The philosophy that drives these standards
- **ARCHITECTURE.md** — The streaming pipeline design
- **CONTRIBUTING.md** — The workflow that enforces these standards
