//! Smoke tests for the subcommand dispatch layer of the docspec binary.
//! Behavioral tests for `convert` live in cli.rs; this file verifies only that
//! the binary correctly routes between subcommands and handles bare invocation.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    clippy::unwrap_used
)]

use assert_cmd::Command;
use predicates::prelude::*;

/// Verify that bare `docspec` invocation prints usage and exits non-zero.
#[test]
fn bare_invocation_requires_subcommand() {
    Command::cargo_bin("docspec")
        .unwrap()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage").or(predicate::str::contains("subcommand")));
}

/// Verify the convert subcommand routes correctly.
#[test]
fn convert_subcommand_routes() {
    Command::cargo_bin("docspec")
        .unwrap()
        .args(["convert", "--from", "markdown", "--to", "html"])
        .write_stdin("Hello world\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello"));
}

/// Verify the http subcommand routes correctly. Uses --help rather than actually binding.
#[cfg(feature = "http")]
#[test]
fn http_subcommand_routes() {
    Command::cargo_bin("docspec")
        .unwrap()
        .args(["http", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--host"))
        .stdout(predicate::str::contains("--port"));
}

/// Verify the http subcommand actually binds and serves /health.
///
/// Uses an in-process retry loop with a bounded deadline rather than a fixed
/// sleep + shelling out to `curl`, so the test is deterministic and has no
/// dependency on an external binary.
#[cfg(feature = "http")]
#[test]
fn http_subcommand_binds_and_serves_health() {
    use core::time::Duration;
    use std::process::Stdio;
    use std::time::Instant;

    // Pick a random ephemeral port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener); // release so docspec http can bind

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_docspec"))
        .args(["http", "--port", &port.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // Poll /health until ready or timeout. Each request has its own short timeout
    // so a slow/missing server cannot stall the loop.
    let url = format!("http://127.0.0.1:{port}/health");
    let deadline = Instant::now() + Duration::from_secs(5);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();

    let body = loop {
        if let Ok(resp) = client.get(&url).send() {
            if resp.status().is_success() {
                break resp.text().unwrap_or_default();
            }
        }
        if Instant::now() >= deadline {
            drop(child.kill());
            drop(child.wait());
            panic!("docspec http /health did not respond within 5s");
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    child.kill().unwrap();
    let wait_result = child.wait();
    assert!(
        wait_result.is_ok(),
        "child process should exit after kill: {:?}",
        wait_result.err()
    );

    assert!(
        body.contains("Healthy"),
        "expected 'Healthy' in body, got: {body}"
    );
}
