//! Smoke tests for Sentry env-var gating — spawns the real binary.

// Reason: test code uses expect/unwrap/panic for test assertions.
#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

use core::time::Duration;
use std::io::{BufRead as _, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

/// Maximum time to wait for the server to log its listening address.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Per-request HTTP timeout for smoke tests. Bounds each request so a hung
/// server fails the test instead of blocking CI indefinitely.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Connect timeout for smoke tests. Tight bound because the server runs locally.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// RAII guard that kills the wrapped child process when dropped, including
/// during stack unwinding from a panicking test assertion. Prevents orphaned
/// `docspec-http` processes from polluting CI ports across runs.
struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _kill = child.kill();
            let _wait = child.wait();
        }
    }
}

/// Builds a reqwest blocking client with bounded request and connect timeouts.
fn smoke_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .expect("build reqwest client")
}

/// Starts the `docspec-http` binary with `--port 0`, custom env vars set and
/// removed, and waits until the bound address appears on stderr.
///
/// Returns the child guard, the bound port, and all stderr lines captured
/// during startup (up to and including the `"listening"` line). Lines emitted
/// before the `"listening"` line — such as the invalid-DSN warning — are
/// therefore included in the returned `Vec`.
#[must_use]
fn start_server_with_env(
    set_vars: &[(&str, &str)],
    remove_vars: &[&str],
) -> (ChildGuard, u16, Vec<String>) {
    let bin = env!("CARGO_BIN_EXE_docspec-http");
    let mut cmd = Command::new(bin);
    cmd.args(["--port", "0"])
        .stderr(Stdio::piped())
        .stdout(Stdio::null());
    for (key, value) in set_vars {
        cmd.env(key, value);
    }
    for key in remove_vars {
        cmd.env_remove(key);
    }
    let mut child = cmd.spawn().expect("failed to start docspec-http");

    let stderr = child.stderr.take().expect("stderr pipe");
    let guard = ChildGuard(Some(child));

    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        // Reason: continue draining stderr to EOF even after the receiver is
        // dropped, so request-time logging cannot fill the OS pipe buffer and
        // block the child process under test.
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _send = tx.send(line);
        }
    });

    let deadline = Instant::now()
        .checked_add(STARTUP_TIMEOUT)
        .expect("deadline overflow");
    let mut captured_lines: Vec<String> = Vec::new();
    let mut port: u16 = 0;

    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        let Ok(line) = rx.recv_timeout(remaining) else {
            break;
        };

        captured_lines.push(line.clone());

        if line.contains("listening") {
            if let Some(parsed) = line
                .split("127.0.0.1:")
                .nth(1)
                .and_then(|after_ip| {
                    after_ip
                        .chars()
                        .take_while(char::is_ascii_digit)
                        .collect::<String>()
                        .parse::<u16>()
                        .ok()
                })
                .filter(|&port_num| port_num != 0)
            {
                port = parsed;
                break;
            }
        }
    }

    let captured_display = captured_lines.join("\n");
    assert_ne!(
        port,
        0,
        "server did not bind within {}s\nstderr captured:\n{captured_display}",
        STARTUP_TIMEOUT.as_secs()
    );

    (guard, port, captured_lines)
}

/// Spawns the binary with no Sentry DSN env vars. Asserts the server starts
/// cleanly and responds to health checks.
#[test]
fn binary_starts_without_dsn() {
    let (_guard, port, captured_lines) =
        start_server_with_env(&[], &["DOCSPEC_SENTRY_DSN", "SENTRY_DSN"]);

    assert!(
        captured_lines
            .iter()
            .any(|line| line.contains("docspec-http listening")),
        "expected 'docspec-http listening' in stderr\ncaptured:\n{}",
        captured_lines.join("\n")
    );

    let client = smoke_client();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .expect("HTTP request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

/// Spawns the binary with `SENTRY_DSN=not-a-dsn`. Asserts that:
/// - the invalid-DSN warning appears in stderr,
/// - the DSN value itself does NOT appear in stderr (G1: never log DSN), and
/// - the server starts and responds to health checks.
#[test]
fn binary_starts_with_invalid_dsn() {
    let (_guard, port, captured_lines) =
        start_server_with_env(&[("SENTRY_DSN", "not-a-dsn")], &["DOCSPEC_SENTRY_DSN"]);

    let captured_joined = captured_lines.join("\n");

    assert!(
        captured_lines
            .iter()
            .any(|line| line.contains("warning: invalid Sentry DSN format; Sentry disabled")),
        "expected invalid-DSN warning in stderr\ncaptured:\n{captured_joined}"
    );

    assert!(
        !captured_joined.contains("not-a-dsn"),
        "DSN value must not appear in stderr (G1)\ncaptured:\n{captured_joined}"
    );

    let client = smoke_client();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .expect("HTTP request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

/// Spawns the binary with a syntactically valid placeholder DSN that points to
/// a closed port. Transport will silently drop events. Asserts that:
/// - the server starts cleanly,
/// - the DSN value does NOT appear in stderr (G1), and
/// - the conversion endpoint returns 200.
#[test]
fn binary_starts_with_placeholder_valid_dsn() {
    let (_guard, port, captured_lines) = start_server_with_env(
        &[("DOCSPEC_SENTRY_DSN", "https://public@127.0.0.1:9/1")],
        &["SENTRY_DSN"],
    );

    let captured_joined = captured_lines.join("\n");

    assert!(
        captured_lines
            .iter()
            .any(|line| line.contains("docspec-http listening")),
        "expected 'docspec-http listening' in stderr\ncaptured:\n{captured_joined}"
    );

    assert!(
        !captured_joined.contains("public@127.0.0.1:9/1"),
        "DSN value must not appear in stderr (G1)\ncaptured:\n{captured_joined}"
    );

    let client = smoke_client();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/conversion"))
        .header("Content-Type", "text/markdown")
        .body("# Hello\n\nWorld")
        .send()
        .expect("HTTP request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

/// Spawns the binary with a placeholder DSN (Sentry active), sends SIGTERM,
/// and asserts that the process exits cleanly within 5 seconds with exit code 0.
#[test]
fn binary_handles_sigterm_cleanly_with_sentry_active() {
    let (mut guard, _port, _captured_lines) = start_server_with_env(
        &[("DOCSPEC_SENTRY_DSN", "https://public@127.0.0.1:9/1")],
        &["SENTRY_DSN"],
    );

    let pid = guard.0.as_ref().expect("child is alive").id();

    // Send SIGTERM via the shell `kill` command — no nix or libc dev-dep needed.
    Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .expect("kill -TERM");

    // Take the child out of the guard so we can wait for it with a deadline.
    // The guard's Drop will see None and skip the kill.
    let mut child = guard.0.take().expect("child is alive");

    let deadline = Instant::now()
        .checked_add(Duration::from_secs(5))
        .expect("deadline overflow");

    loop {
        if let Some(status) = child.try_wait().expect("wait for child") {
            assert_eq!(
                status.code(),
                Some(0),
                "expected exit code 0 after SIGTERM, got: {status:?}"
            );
            return;
        }
        if Instant::now() > deadline {
            let _kill = child.kill();
            let _wait = child.wait();
            panic!("process did not exit within 5s after SIGTERM");
        }
        thread::sleep(Duration::from_millis(50));
    }
}
