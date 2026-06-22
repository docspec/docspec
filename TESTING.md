# Testing Philosophy

Testing is not a step that happens after development. It is development. Every feature, every bug fix, every refactor comes with tests that prove it works. Untested code is incomplete code.

We target 98% test coverage for new and changed executable Rust lines in crates covered by the standard coverage job. Not as an aspirational goal. As a floor. The 2% tolerance exists for genuinely untestable code: platform-specific initialization, embedded hardware interaction, code that interfaces with external systems beyond our control, and WASM/CLI entry points that require integration testing. If your code does not have tests, it is not done.

## Why 98%

Coverage is not a perfect metric. You can have 100% coverage and still have bugs. But low coverage guarantees bugs — code that has never been executed by a test has never been verified. High coverage forces you to think about every path, every edge case, every failure mode.

Going from 80% to 98% coverage catches the subtle bugs, the edge cases, the error paths that only trigger in production. These are the bugs that corrupt data. These are the bugs that crash servers. These are the bugs we refuse to ship.

98% is not perfectionism. It is discipline. It says we do not ship code we have not thought carefully about. It says we respect our users enough to verify our work.

The 2% gap is a recognition that some code lives at the boundary between our system and the outside world. Code that talks to hardware. Code that interfaces with the operating system. Code that handles signals we cannot simulate. Everything else gets tested.

We track coverage religiously. Pull requests enforce 98% coverage on executable Rust lines added or changed by the branch in crates covered by the standard coverage job. The full covered-crate report remains visible in local and CI logs as baseline information. We question any drop in coverage and ratchet the baseline upward over time. Coverage is a team metric, not an individual one.

High coverage is not achieved by writing bad tests that cover lines without asserting anything. Coverage is meaningful only when tests are meaningful. Write assertions. Check the output. Verify the error. Test the edge case. Coverage is a side effect of thoroughness, not the goal itself. A line that is covered but not asserted is not really tested.

## Test-Driven Development

TDD is our preferred approach: write a failing test first, then write the minimum code to make it pass, then refactor. This sequence forces you to think about the interface before the implementation. It prevents over-engineering.

TDD is preferred, not mandated. What is mandated is coverage. Tests may come during or after implementation, but they must come before the PR is merged.

When you practice TDD, you discover the interface through use. The test becomes a design tool, not just a verification tool. But we accept that not every problem yields to this approach. Write the tests when you understand the problem.

The TDD cycle is red, green, refactor. Red: write a test that fails. Green: write the minimum code to make it pass. Refactor: clean up the code while keeping tests passing. This rhythm produces clean, tested code and prevents technical debt.

Code written with TDD tends to be more modular with clearer interfaces and better testability. These properties make the code easier to maintain, extend, and understand.

## Test Types

We use multiple test types because no single approach catches every kind of bug. Each type has a purpose. Each type finds different problems.

### Unit Tests

Unit tests verify individual components in isolation. A parser for a single XML element. A function that converts an event to a string. A utility that escapes special characters. Isolated, fast, focused. They catch logic errors at the component level.

A good unit test has a clear purpose. It tests one thing. It has a descriptive name that explains what is being verified. It sets up minimal state and runs quickly.

Unit tests are the foundation. They are the majority of our test suite. They run on every build in milliseconds, giving immediate feedback when something breaks.

Write unit tests for pure functions, stateless transformations, algorithms, and anything that can be tested in isolation. The more you can test in isolation, the faster your tests run and the clearer your failures are.

Unit tests are also documentation. They show how a function is supposed to be called, what inputs are valid, and what outputs to expect.

### Integration Tests

Integration tests verify components working together. A reader that produces events from a real document format. A writer that produces valid output. A pipeline that connects reader to writer and produces the expected result. They catch interface errors and unexpected interactions.

Integration tests are slower than unit tests and involve more setup, but they find the bugs that unit tests miss: mismatched assumptions between components, subtle incompatibilities in data formats, and ordering problems.

Integration tests use real dependencies and verify that the system works as a whole. They are also the best place to verify error handling, ensuring that errors propagate correctly through the pipeline.

### Snapshot Tests

Snapshot tests capture the exact output of a conversion and alert on any change. They are invaluable for regression testing: if the output changes, a snapshot test fails, forcing a deliberate decision about whether the change is intentional.

Snapshots are change detectors. When a snapshot test fails, you must decide: is this change correct? If yes, update the snapshot. If no, fix the code.

We use snapshots for output formats that are stable and well-defined: HTML output, Markdown rendering, JSON serialization. Snapshots make changes visible in code review and provide a history of how output has evolved.

Snapshots also serve as regression tests. When a bug is fixed, we add a snapshot of the correct output. If the bug ever returns, the snapshot test will fail.

### Snapshot Review

Snapshot files live in `tests/snapshots/docx/pandoc/` and are committed alongside the fixtures they cover. Run `cargo insta review` to interactively inspect pending snapshots: press `a` to accept a change as correct, `r` to reject it and keep the existing snapshot. Accept snapshots only when you have verified the new output is correct — not merely different.

To generate snapshots for the first time (or for fixtures added since the last run), use `INSTA_UPDATE=unseen cargo test --test pandoc_corpus -p docspec-docx-reader`. This writes only the missing snapshots and leaves existing ones untouched. In CI (`CI=true`, `INSTA_UPDATE` unset), any missing or mismatched snapshot causes the test to fail immediately — do not set `INSTA_UPDATE=always` in any committed script or workflow.

### Fuzz Tests

Fuzz tests send random, malformed, and adversarial inputs at the parsers. The goal is simple: do not crash. A parser must handle any byte sequence without panicking.

Fuzzing is about robustness, not correctness of output. A malformed document should produce an error, not a crash. An unexpected byte sequence should be rejected gracefully.

We run fuzzers continuously. They generate millions of inputs and find edge cases we would never think to test. Fuzzing hardens the code against the real world, where inputs are not always well-formed.

Fuzzing is especially important for document parsers. Documents come from everywhere. They are corrupted in transmission, malformed by buggy software, and crafted by attackers. Fuzzing ensures our parsers handle all of these gracefully.

### Roundtrip Tests

Roundtrip tests convert a document from format A to format B and back to format A, verifying the result matches the original. They catch semantic loss in conversion pipelines.

Not all conversions are reversible. Some formats lose information, and some formats only have readers (no writers). But where roundtripping is possible, we test it. An HTML file converted to Markdown and back to HTML should preserve semantic content.

Roundtrip tests validate our understanding of the formats. If we cannot roundtrip a document, we may not fully understand the format. The test forces us to learn and handle edge cases we might otherwise miss.

## Test Data and Fixtures

Tests use representative documents from real-world sources. Not toy examples. Real DOCX files from word processors. Real ODT files with complex formatting. Real HTML from actual websites.

Fixtures are checked into the repository. They are first-class artifacts, not afterthoughts. When a bug is found in production, the first step is to create a fixture that reproduces it. The fixture stays in the repository forever, preventing regression.

A good fixture is minimal but complete. It contains the minimum structure needed to reproduce the issue, but it is a valid document. It has a descriptive name and lives in the fixtures directory, organized by format and purpose.

We do not generate fixtures programmatically unless the generation itself is under test. Real files from real sources are preferred. They contain the complexity that synthetic data misses.

Fixtures are documentation too. They show what kinds of documents we handle and demonstrate the complexity we support.

## The Fail-Fast Test Principle

Tests fail loudly and specifically. A failing test tells you exactly what failed: which input, which output, what was expected, what was received. Vague test failures are not acceptable.

Good assertions are specific. They check exact values, not just presence or absence. They compare the full output, not just a substring. They fail with messages that explain the context.

When a test fails, you should know immediately what went wrong. You should not need to add print statements or run the test again with debugging enabled. The failure message should be enough.

Specific failures save time and prevent cascading failures. When a test fails early, you fix it before moving on.

## Exact-Value Assertions

Assertions check exact values. Substring matches, type-only checks, and structural-shape checks all hide changes the test did not intend to allow. A test that accepts more than the contract specifies is a test that has stopped doing its job.

The rule is simple. For any value a test inspects, assert the entire expected value. Response bodies are compared byte-for-byte or as exact JSON values. Structured types are compared as a whole, not field by field. Enums with inner data are pinned to the inner value, not just the variant. Empty bodies are asserted explicitly.

A handful of patterns are banned outright. Substring matches on response bodies (`body.contains(...)`) accept any superset of the expected text. Type-only checks (`json.is_array()`, `value.is_string()`) accept any value of the right shape. Structural-shape checks on JSON objects (`obj.contains_key(...)` paired with `obj.len()`) accept any object with the right keys but say nothing about the values. Custom shape-checking helpers (`assert_problem_json(...)`) hide the actual contract from the call site. Negative substring checks (`!detail.contains("secret")`) prove only that one substring is absent, not that the content is what was expected. Pattern matches with bare wildcards (`matches!(result, Variant { .. })`) accept any inner value. Each of these can be replaced by an exact-equality assertion that is strictly stronger.

For request/response tests specifically, every test asserts three things: the exact status code, the exact relevant headers, and the exact body. A response that carries no body asserts the body is empty. A test that asserts only status, only headers, or only one field of a JSON body is incomplete.

There are four narrow exceptions where non-exact assertions are permitted:

- Generated values — UUIDs, timestamps, ports assigned by the operating system. Assert format or parseability, not the exact value. `Uuid::parse_str(...)` followed by a version check is sufficient.
- Absence — `is_none()` and `is_empty()` are exact assertions when the contract is "must not exist".
- Unit-variant results — `result.is_ok()` is the maximally tight assertion when the `Ok` variant carries no inner value.
- Types without `PartialEq` — `matches!` patterns are permitted, but the inner fields must be pinned with `if` guards. Bare `{ .. }` patterns are banned.

If a test cannot assert the exact value, the test documents why in a comment. Otherwise, the test writes the exact assertion.

## Test as Documentation

A test is a specification. It says "given this input, I expect this output." Reading the tests tells you how the code is supposed to work. Test names matter: "converts_nested_list_to_html" is better than "test_1".

Write tests that are readable. Another developer should be able to understand what is being tested without reading the implementation. The test should tell a story: here is the setup, here is the action, here is the expected result.

Tests are the first documentation a new contributor reads. They show how the code is used, demonstrate the API, and provide examples that compile and run.

When documentation and tests disagree, the tests are right. Update the documentation to match. Or update the tests if the behavior is wrong. But do not let them diverge.

Readable tests use helper functions to reduce boilerplate and descriptive variable names. Structure tests in three parts: arrange, act, assert. This structure makes tests easy to scan and understand.

Tests should be independent. Each test should set up its own state and clean up after itself. Tests should not depend on the order in which they run.

## Testing and Refactoring

Tests enable refactoring. Without tests, refactoring is dangerous. With tests, you change code and know. The tests are the safety net.

Refactoring without tests is not refactoring. It is rewriting. It is risky. It introduces bugs.

When you refactor, run the tests. All of them. If they pass, your refactoring preserved behavior. If they fail, you made a mistake. Fix it.

Good tests are independent of implementation details. They test behavior, not structure. When you refactor, the tests should continue to pass.

Refactoring is how we keep the codebase healthy. Tests are what make refactoring possible. Together, they enable continuous improvement without fear.

## Testing in the Embedded Context

DocSpec runs on microcontrollers with 512 KB of RAM. Tests must respect this constraint. We do not allocate huge buffers in tests. We do not load massive fixtures. We test with the same discipline we apply to production code.

Tests run on the target hardware when possible. They run in constrained environments. They verify that the code works within the limits it was designed for.

Memory-conscious testing means using small fixtures, cleaning up after tests, and not leaking resources. Write tests that would run on a microcontroller.

Embedded tests also verify timing constraints and memory usage stays within bounds.

## The Testing Mindset

Testing is not a chore. It is a way of thinking about code. It is the discipline of verification.

When you write code, ask: how do I know this works? When you fix a bug, ask: how do I know this stays fixed? When you review code, ask: where are the tests?

The testing mindset is skeptical. It does not trust. It verifies. It assumes code is wrong until proven otherwise. This skepticism produces better software.

Testing is a skill. It improves with practice. Write more tests. Read tests. Review tests. Learn what makes a test good.

Good testers are skeptical. They question assumptions. They look for edge cases. They think about what could go wrong. This mindset is valuable beyond testing.

We are memory extremists. We are also testing extremists. We do not ship code we have not verified. We do not trust code we have not tested.

Testing is how we honor our users and the craft. It is how we build software that lasts. Every test is a promise that the code works and will keep working.

The test suite is our safety net. It catches us when we fall. It gives us the courage to refactor and the confidence to release.

Quality is not an accident. It is the result of discipline, care, and verification. Testing is the verification. It is how we know we have done things right.
