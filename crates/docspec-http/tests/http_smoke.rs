//! Smoke tests that spawn the real `docspec-http` binary and exercise it
//! with real HTTP requests.
//!
//! Run with `--test-threads=1` to avoid port conflicts between tests.

// Reason: test code uses expect/unwrap to assert expected-Ok results;
// panicking here indicates a test bug, not a runtime error.
// Reason: integration test files (tests/*.rs) by design contain #[test] functions
// outside any #[cfg(test)] module — this is the standard Cargo integration test structure.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::tests_outside_test_module
)]

use std::io::{BufRead as _, BufReader};
use std::process::{Child, Command, Stdio};

/// Starts the `docspec-http` binary with `--port 0` and waits until the
/// bound address appears on stderr.
///
/// Reads lines from the server's stderr until a line containing both
/// "listening" and the server address is found. The tracing pretty format
/// embeds ANSI escape codes around field names but not around the IP:port
/// value itself, so the parser splits on the literal `"127.0.0.1:"` prefix
/// to reach the raw port digits without needing ANSI stripping.
#[must_use]
fn start_server() -> (Child, u16) {
    let bin = env!("CARGO_BIN_EXE_docspec-http");
    let mut child = Command::new(bin)
        .args(["--port", "0"])
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to start docspec-http");

    let stderr = child.stderr.take().expect("stderr pipe");
    let reader = BufReader::new(stderr);
    let mut port: u16 = 0;

    for result in reader.lines() {
        let line = result.expect("read stderr line");
        if line.contains("listening") {
            let parsed = line
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
                .filter(|&p| p != 0);
            if let Some(p) = parsed {
                port = p;
                break;
            }
        }
    }

    assert_ne!(
        port, 0,
        "failed to parse bound port from log; server may not have started"
    );
    (child, port)
}

fn stop_server(mut child: Child) {
    let _kill = child.kill();
    let _wait = child.wait();
}

#[test]
fn smoke_post_conversion() {
    let (child, port) = start_server();
    let url = format!("http://127.0.0.1:{port}/conversion");
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(&url)
        .header("Content-Type", "text/markdown")
        .body("# Hello\n\nWorld")
        .send()
        .expect("HTTP request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/vnd.docspec.blocknote+json; charset=utf-8"
    );
    assert!(resp.headers().contains_key("x-request-id"));
    assert_eq!(
        resp.headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "max-age=0, private, must-revalidate"
    );
    let body: serde_json::Value = resp.json().expect("JSON body");
    assert!(body.is_array(), "response should be JSON array");
    stop_server(child);
}

#[test]
fn smoke_get_health() {
    let (child, port) = start_server();
    let url = format!("http://127.0.0.1:{port}/health");
    let client = reqwest::blocking::Client::new();
    let resp = client.get(&url).send().expect("HTTP request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/plain; charset=utf-8"
    );
    assert_eq!(resp.text().unwrap(), "Healthy.");
    stop_server(child);
}

#[test]
fn smoke_head_health() {
    let (child, port) = start_server();
    let url = format!("http://127.0.0.1:{port}/health");
    let client = reqwest::blocking::Client::new();
    let resp = client.head(&url).send().expect("HTTP request");
    assert_eq!(resp.status(), reqwest::StatusCode::NO_CONTENT);
    stop_server(child);
}

#[test]
fn smoke_port_zero_logs_actual_port() {
    // Verifies the port-0 logging fix: the logged address must NOT be ":0".
    // start_server() already asserts port != 0 after parsing from log.
    let (child, port) = start_server();
    assert_ne!(port, 0, "server must log non-zero port");
    let url = format!("http://127.0.0.1:{port}/health");
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(&url)
        .send()
        .expect("server reachable on logged port");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    stop_server(child);
}

#[test]
fn smoke_error_response_has_cache_control() {
    let (child, port) = start_server();
    let url = format!("http://127.0.0.1:{port}/unknown");
    let client = reqwest::blocking::Client::new();
    let resp = client.get(&url).send().expect("HTTP request");
    assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(
        resp.headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "max-age=0, private, must-revalidate"
    );
    stop_server(child);
}
