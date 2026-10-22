# Security Principles

Security in DocSpec is not a separate concern. It flows from our core values: memory safety, fail-fast error handling, defensive parsing, and minimal dependencies. This document explains how those values translate into concrete security properties. It is not a threat model, vulnerability policy, or security audit. It is how we think about security.

---

## 1. Memory Safety as the Foundation

DocSpec is written in Rust with `unsafe_code` forbidden. Memory safety is not a feature we added. It is the default. Buffer overflows, use-after-free, data races: these entire categories of vulnerability are eliminated by construction.

This matters for a document conversion library. Document formats are complex. Parsers for complex formats have historically been fertile ground for memory corruption vulnerabilities. We eliminate this risk at the language level.

The manifesto says: "The compiler is our ally. We do not bypass it." This applies doubly to security. Every bounds check is enforced. Every lifetime is verified. Every pointer is valid or it does not compile.

Memory safety is our first line of defense. It is not perfect — logic errors still exist — but it removes the most common and most dangerous class of security bugs entirely. An attacker cannot exploit a buffer overflow in our code because the language makes buffer overflows impossible.

This is why we forbid unsafe code completely. There are no exceptions. No special cases. If you cannot do it safely, you do not do it. Period.

The choice of Rust is a security decision. Other languages could work. Rust works best because it gives us safety without sacrificing performance. We do not need to choose between fast and safe. We get both.

---

## 2. Error Handling as a Security Boundary

How errors are reported depends on the context. The principle: do not leak internal details to untrusted surfaces.

For CLI tools: the user is a developer or administrator. More detail is appropriate. Full error messages including context help diagnose problems. Stack traces can be enabled with flags. File paths are shown because the user needs to know which file failed.

For library users: the caller receives typed error values. They decide what to show to their users. The library does not make this decision for them. A library user might be building a web service, a desktop app, or an embedded system. Each has different security requirements. We provide the error; they handle the disclosure.

The general rule: the closer to an untrusted surface, the less detail the error contains. Information leakage is a vulnerability. Error messages that reveal internal structure help attackers craft better attacks. We do not help attackers.

For the HTTP API: a 4xx problem response's `detail` describes the caller's request or document ([RFC 9457 §3.1.4](https://www.rfc-editor.org/rfc/rfc9457#section-3.1.4)). A 5xx `detail` is a fixed string; the underlying cause is logged and sent to Sentry, never returned to the client ([RFC 9457 §5](https://www.rfc-editor.org/rfc/rfc9457#section-5)).

---

## 3. Input Validation Philosophy

All input is untrusted. Every byte of every document comes from outside the process — from a user, a network, a filesystem. We parse defensively.

Parsers fail fast on malformed input rather than attempting recovery. A malformed ZIP structure in a DOCX file returns an error immediately. No attempt to "guess" at the correct structure. No partial parsing that could produce wrong output that looks correct.

This is the "fail fast" principle from our manifesto applied to security. A failed conversion is better than a wrong conversion. Wrong conversions propagate. They corrupt databases. They break trust. When the input is broken, we say so immediately.

Parsers are hardened against malicious input through continuous fuzzing. Fuzz tests send random and adversarial byte sequences at every parser. The goal: no input causes a crash, a panic, or undefined behavior. The fuzz corpus grows as new edge cases are discovered.

Fuzzing runs in CI on every commit. It runs for hours, not minutes. We use coverage-guided fuzzing to find inputs that exercise new code paths. We track the corpus over time. A crash in fuzzing is treated as a security bug, even if it seems hard to exploit. We fix it.

Document formats are attack surfaces. A malicious DOCX could try to exploit a ZIP parser. A malformed HTML document could try to cause excessive memory use. Our parsers are designed to handle these cases gracefully. They validate. They limit. They fail safely.

---

## 4. Resource Limits

Limits exist where a document can make the process grow without bound. Each one fails fast with an error that names the limit, answers a specific HTTP status, and exits with a specific code on the command line. Nothing here is a suggestion: every row is enforced in code and pinned by a test.

### Always on

| Limit | Value | Where | HTTP | CLI exit |
| --- | --- | --- | --- | --- |
| DOCX package part size (`_rels`, `styles.xml`, `numbering.xml`, `document.xml.rels`) | 64 MiB decompressed | `docspec-docx-reader` | `422` `resource-limit` | 65 |
| DOCX `document.xml` single-node size | 64 MiB per node | `docspec-docx-reader` | `422` `resource-limit` | 65 |
| DOCX table nesting depth | 100 | `docspec-docx-reader` | `422` `resource-limit` | 65 |
| List nesting level | clamped to 8 (deeper levels are flattened, not rejected) | `docspec-docx-reader` | — | — |

The part and node caps stop compressed-size attacks (a 2 MB "zip bomb" that inflates to gigabytes) before the inflated bytes are held. The depth cap stops a 6 KB file from overflowing the stack.

### Opt-in, configured per server

| Limit | Flag | HTTP |
| --- | --- | --- |
| Request body size | `docspec http --max-body-size <BYTES>` | `413 Content Too Large`, before the body is buffered when `Content-Length` says so |
| Conversion time | `docspec http --conversion-timeout <SECONDS>` | `503 Service Unavailable` with `Retry-After`; the conversion is stopped at its next event, in the worker thread, not merely abandoned |

Both default to unlimited. An operator who exposes the server to untrusted callers should set both; the README's deployment notes say so.

### Not bounded

- The HTTP server holds the accepted request body and the whole conversion output in memory for the duration of a request. Memory per request is proportional to input plus output size.
- A single wide DOCX table is read into the reader's event queue before its first event is yielded, so memory is proportional to the largest table (tracked as issue #415). The caps above bound its parts and depth, not its width.
- There is no idle or read timeout at the HTTP layer; use a reverse proxy.

The streaming architecture is what makes the rest of a document cost constant memory: events flow one at a time from reader to writer. The limits above exist for the places where a format lets one node, one part, one nesting chain or one table defeat that.

---

## 5. Dependency Security

Every dependency is a potential attack surface. We minimize the dependency count deliberately. We audit dependencies with cargo-deny, checking for known security advisories. Dependencies from unknown registries or unverified git sources are blocked.

When a security advisory is published for a dependency we use, we update within 24 hours. Non-critical advisories are tracked and resolved in the next scheduled release.

The manifesto says: "Dependencies are not free. They are liabilities." This is especially true for security. Each dependency is code we did not write but must trust. We keep that trust surface small.

Our cargo-deny configuration:
- Blocks dependencies with known security advisories
- Blocks dependencies from unknown registries
- Blocks git dependencies that are not explicitly allowed
- Requires license verification for all dependencies

We review dependencies before adding them. We ask: what is the security track record? How quickly do they fix vulnerabilities? How many transitive dependencies do they bring? A dependency with 50 sub-dependencies is 50 times more attack surface. We avoid such dependencies when possible.

Vendoring is our friend. When a dependency is small and critical, we consider vendoring it. This means copying the code into our repository. We trade update convenience for control. We can review the code. We can fix issues ourselves. We are not dependent on upstream responsiveness.

---

## 6. Reporting Vulnerabilities

If you find a security vulnerability, do not open a public issue. Contact the maintainers privately at security@docspec.dev.

We will work with you to understand the vulnerability, develop a fix, and coordinate disclosure. Our goal is to protect users while giving you credit for the discovery.

We commit to:

- Acknowledging receipt within 48 hours
- Providing a timeline for the fix within 7 days
- Keeping you informed of progress
- Coordinating disclosure when the fix is released

Public disclosure before a fix is available helps attackers, not users. Please give us time to do this right.

When you report a vulnerability, please include:
- A description of the vulnerability
- Steps to reproduce (if possible)
- The impact you believe it has
- Whether you have found it in the wild or through analysis

We appreciate security researchers. We will not take legal action against researchers who follow responsible disclosure. We will not blame you for finding our mistakes. We will thank you and fix the issue.

Security researchers who report valid vulnerabilities will be acknowledged in our release notes and security advisories, unless they prefer to remain anonymous. We believe in giving credit where credit is due.

---

## Summary

Security in DocSpec flows from our core values:

- Memory safety through Rust's type system
- Defensive parsing through fail-fast validation
- Least-privilege error handling through context-aware reporting
- Resource protection through enforced limits
- Supply chain security through minimal, audited dependencies

We build secure software by building correct software. The two are the same. Every principle in our manifesto — fail fast, no unsafe code, strict quality — contributes to security.

We do not claim to be perfect. Vulnerabilities may exist. When they are found, we fix them quickly. We learn from them. We improve.

This is our commitment to our users: we take security seriously. We design for it. We test for it. We respond to it. You can trust DocSpec with your documents because we have built it to be trustworthy.

---

## Related Documents

- **[MANIFESTO.md](MANIFESTO.md)** — Our core philosophy and values
- **[ARCHITECTURE.md](ARCHITECTURE.md)** — Technical design and streaming pipeline
- **[CODING_STANDARDS.md](CODING_STANDARDS.md)** — Code quality and review standards

These documents work together. The manifesto explains why we care about security. The architecture explains how we implement it. The standards explain how we maintain it.
