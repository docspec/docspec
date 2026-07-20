# Bug Triage & Reporting

A bug report is a reproduction handed to us; triage is how we turn a stream of reports into a queue we can actually work. This guide has two halves: the first is for anyone **reporting** a bug, the second is the **triage** runbook for maintainers. Both lean on the same values as the rest of DocSpec — fail fast, stay honest about what we support, and prove every fix with a test. (See the [Manifesto](MANIFESTO.md) for why.)

---

## Reporting a bug

### First, the one exception: security

If the bug is a **security vulnerability** — a parser that panics or hangs on crafted input, a document that exhausts memory or CPU, anything that lets a malicious document affect the host — **do not open a public issue.** Report it privately through the process in [SECURITY.md](SECURITY.md). Public disclosure before a fix helps attackers, not users. When in doubt about whether something is security-sensitive, treat it as security and report privately.

### Before you file

- **Search first.** Someone may have already reported it. Add to the existing issue instead of opening a duplicate.
- **Check you're current.** Reproduce on the latest release (or `main`) before filing — the bug may already be fixed.
- **Check it's actually a bug.** DocSpec is honest about its limits: some inputs are documented as unsupported and some content is knowingly dropped (for example, the HTML reader and writer are paragraph-only today). Skim the relevant crate README before filing. If we drop something *without* documenting it, that itself is a bug — tell us.
- **Reduce it.** The most useful thing you can send is the *smallest* document that still reproduces the problem. A ten-page report that fails is a mystery; a three-line document that fails is a test case.

### What makes a report actionable

Open a **Bug Report** issue on [`docspec/docspec`](https://github.com/docspec/docspec/issues/new/choose) and fill in the form. The fields exist for a reason:

- **Version** — the release, tag, or commit. "Latest" ages badly; a version number does not.
- **Description** — what happened, and what you expected instead.
- **Steps to reproduce** — the exact command or request. For the CLI, the full `docspec convert …` line. For the HTTP API, the `Content-Type`, the `Accept` header, and the endpoint.
- **Sample document** — the minimal input that triggers it, and the input/output formats involved. For a conversion bug this is the single most valuable field; without it we are guessing.
- **Error output / logs** — the exact error text, not a paraphrase.
- **Environment** — OS, Rust version (`rustc --version`), and DocSpec version.

A report that lets us reproduce the bug from one paste is a report we can fix today. A vague one waits while we ask for what the form already requested.

---

## Triaging a bug (maintainers)

Every new bug arrives labelled `bug` and `triage`. Triage is the pass that earns the removal of `triage`: confirm the report, classify it, route it, and decide when it gets worked. Do it promptly — an untriaged queue is a queue no one trusts.

### The triage pass

1. **Reproduce.** Follow the steps. If you can reproduce, say so on the issue. If you can't — missing version, no sample document, unclear steps — ask for exactly what's missing and label `needs-info` (or `needs-repro`). The reporter's minimal sample becomes the regression fixture later, so insist on it.
2. **Confirm it's a bug.** If the behaviour is a documented limitation or a silent drop we already warn about, it isn't a bug — redirect it (a feature request for the missing capability, or a docs fix if the limitation was *not* actually documented). If it's a request for a new format or feature, relabel `enhancement` and drop `bug`.
3. **Check for security.** If the "bug" is a crash, hang, or resource exhaustion triggered by crafted or malicious input — especially anything reachable through the HTTP API — stop treating it as public. Crashes on ordinary valid input remain public high-severity bugs handled through normal triage (see the Severity table below). Move the crafted-input cases to the private channel in [SECURITY.md](SECURITY.md) and close the public issue with a pointer there, not a description of the flaw. A fuzzing crash counts even if it looks hard to exploit.
4. **Set severity.** Use the model below.
5. **Route to a crate.** Label the area so the right code owner sees it (see [Routing by area](#routing-by-area)).
6. **Prioritise, then drop `triage`.** Decide whether it's next, soon, or someday, and remove the `triage` label so the issue leaves the intake queue.

### Severity

Severity follows our values, not a generic ladder. The worst bug is the one that lies to the user.

| Severity | What it is | Examples |
| --- | --- | --- |
| **Critical** | **Silent wrong output.** The conversion succeeds but the result is corrupt or semantically wrong, with no error. The worst class, because wrong output propagates — it lands in databases and downstream documents before anyone notices. | Text reordered or dropped from otherwise-supported content; mis-encoded characters; a writer emitting structurally wrong JSON that still parses. |
| **High** | **Crash or hang on ordinary input.** A panic, abort, or infinite loop violates fail-fast and the no-panic rule. If the trigger is *crafted* input, it's a security bug — route it privately instead. | A panic converting a valid DOCX; the HTTP server hanging on a well-formed request. |
| **High** | **Unbounded resource use** on input that should hit a limit. If it's remotely triggerable, treat it as security. | A document that exhausts memory or never terminates. |
| **Medium** | **Loud, wrong failure.** It fails visibly when it should succeed, or returns the wrong error. Incorrect, but neither silent nor a crash. | A valid conversion rejected with a spurious parse error; the wrong HTTP status code. |
| **Low** | **Cosmetic or ergonomic.** Confusing messages, doc mismatches, minor output nits that don't change meaning. | An unhelpful error string; a stray blank line in Markdown output. |

Exactly one severity per bug. When a bug spans tiers, take the higher one — a crash that also corrupts output is Critical.

### Routing by area

Route each bug to the crate that owns the behaviour, so the label points at the code:

- A reader bug (input parsing) → `docspec-docx-reader`, `docspec-html-reader`, or `docspec-markdown-reader`.
- A writer bug (output generation) → `docspec-html-writer`, `docspec-markdown-writer`, `docspec-blocknote-writer`, `docspec-oxa-writer`, or `docspec-pandoc-native-writer`.
- Pipeline, event model, or trait behaviour → `docspec-core`.
- The `docspec convert` command → `docspec-cli`; the HTTP surface → `docspec-http`; WASM bindings → `docspec-wasm`.
- Wrong in both directions? Route to the reader first — a reader that emits the wrong events makes every writer look broken.

### Labels

Three labels exist today: `bug` and `triage` (applied by the Bug Report form) and `enhancement`. Triage adds three lean dimensions on top; adopt them as repository labels:

- **Severity** — `severity:critical` … `severity:low`, exactly one per bug.
- **Area** — `area:<crate>` (e.g. `area:docspec-docx-reader`), pointing at the owning crate above.
- **Status** — `needs-info`, `needs-repro`, `confirmed`, `blocked`, as the issue moves.

Keep the set small. A label that changes no decision is noise.

### From confirmed to closed

A triaged bug moves confirmed → in progress → fixed → closed. Two rules hold at the end:

- **Every fix ships with a regression test.** When we fix a bug we add the reporter's minimal sample as a fixture, so the same bug can never return silently. This is the discipline in [TESTING.md](TESTING.md) — a fixed bug without a test is a bug waiting to come back.
- **Close with a reason.** Fixed (link the PR), working-as-documented (link the docs), duplicate (link the original), or can't-reproduce after a fair wait on `needs-info`. Close kindly and reopen freely when new information arrives — a closed issue is not a verdict on the reporter.

---

## Related

- [SECURITY.md](SECURITY.md) — the private channel for vulnerabilities, and why we treat all input as untrusted
- [TESTING.md](TESTING.md) — why every fix needs a regression fixture, and the test types that catch bugs
- [CONTRIBUTING.md](CONTRIBUTING.md) — the branch, commit, and PR workflow a fix follows
- [MANIFESTO.md](MANIFESTO.md) — fail fast, and stay honest about what we support
