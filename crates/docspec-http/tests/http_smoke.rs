//! Smoke tests that spawn the real `docspec-http` binary and exercise it
//! with real HTTP requests.
//!
//! Each test binds to an OS-assigned ephemeral port (`--port 0`), so tests
//! can run in parallel without port conflicts.

// Reason: test code uses expect/unwrap to assert expected-Ok results;
// panicking here indicates a test bug, not a runtime error.
// Reason: integration test files (tests/*.rs) by design contain #[test] functions
// outside any #[cfg(test)] module — this is the standard Cargo integration test structure.
// Reason: harness panics on server-startup timeout to surface the captured stderr.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::tests_outside_test_module,
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

/// Starts the `docspec-http` binary with `--port 0` and waits until the
/// bound address appears on stderr.
///
/// Reads lines from the server's stderr via a background thread that forwards
/// them through an `mpsc` channel, allowing the main thread to enforce a
/// [`STARTUP_TIMEOUT`] deadline. On timeout, the child is killed and the
/// captured stderr is included in the panic message.
///
/// The tracing pretty format embeds ANSI escape codes around field names but
/// not around the IP:port value itself, so the parser splits on the literal
/// `"127.0.0.1:"` prefix to reach the raw port digits without needing ANSI
/// stripping.
#[must_use]
fn start_server() -> (ChildGuard, u16) {
    let bin = env!("CARGO_BIN_EXE_docspec-http");
    let mut child = Command::new(bin)
        .args(["--port", "0"])
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("failed to start docspec-http");

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
    let mut captured = String::new();
    let mut port: u16 = 0;

    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        let Ok(line) = rx.recv_timeout(remaining) else {
            break;
        };
        captured.push_str(&line);
        captured.push('\n');

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
                .filter(|&p| p != 0)
            {
                port = parsed;
                break;
            }
        }
    }

    assert_ne!(
        port,
        0,
        "server did not bind within {}s\nstderr captured:\n{captured}",
        STARTUP_TIMEOUT.as_secs()
    );

    (guard, port)
}

#[test]
fn smoke_post_conversion() {
    let (_guard, port) = start_server();
    let url = format!("http://127.0.0.1:{port}/conversion");
    let client = smoke_client();
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
    let request_id = resp
        .headers()
        .get("x-request-id")
        .expect("x-request-id present")
        .to_str()
        .expect("ASCII header value");
    let parsed = uuid::Uuid::parse_str(request_id).expect("valid UUID");
    assert_eq!(parsed.get_version(), Some(uuid::Version::Random));
    assert_eq!(
        resp.headers()
            .get("cache-control")
            .unwrap()
            .to_str()
            .unwrap(),
        "max-age=0, private, must-revalidate"
    );
    let body: serde_json::Value = resp.json().expect("JSON body");
    assert_eq!(
        body,
        serde_json::json!([
            {
                "type": "heading",
                "props": { "level": 1, "textAlignment": "left" },
                "content": [{ "type": "text", "text": "Hello", "styles": {} }],
                "children": [],
            },
            {
                "type": "paragraph",
                "props": { "textAlignment": "left" },
                "content": [{ "type": "text", "text": "World", "styles": {} }],
                "children": [],
            },
        ])
    );
}

#[test]
fn smoke_get_health() {
    let (_guard, port) = start_server();
    let url = format!("http://127.0.0.1:{port}/health");
    let client = smoke_client();
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
}

#[test]
fn smoke_head_health() {
    let (_guard, port) = start_server();
    let url = format!("http://127.0.0.1:{port}/health");
    let client = smoke_client();
    let resp = client.head(&url).send().expect("HTTP request");
    assert_eq!(resp.status(), reqwest::StatusCode::NO_CONTENT);
    assert_eq!(resp.text().expect("body read"), "");
}

#[test]
fn smoke_port_zero_logs_actual_port() {
    // Verifies the port-0 logging fix: the logged address must NOT be ":0".
    // start_server() already asserts port != 0 after parsing from log.
    let (_guard, port) = start_server();
    assert_ne!(port, 0, "server must log non-zero port");
    let url = format!("http://127.0.0.1:{port}/health");
    let client = smoke_client();
    let resp = client
        .get(&url)
        .send()
        .expect("server reachable on logged port");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

#[test]
fn smoke_error_response_has_cache_control() {
    let (_guard, port) = start_server();
    let url = format!("http://127.0.0.1:{port}/unknown");
    let client = smoke_client();
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
    let body: serde_json::Value = resp.json().expect("JSON body");
    assert_eq!(
        body,
        serde_json::json!({
            "type": "about:blank",
            "title": "Not Found",
            "status": 404,
            "detail": "No route matches GET /unknown",
        })
    );
}

#[test]
fn smoke_metrics_endpoint_returns_prometheus_format() {
    let (_guard, port) = start_server();
    let base = format!("http://127.0.0.1:{port}");
    let client = smoke_client();

    let health = client
        .get(format!("{base}/health"))
        .send()
        .expect("health request");
    assert_eq!(health.status(), reqwest::StatusCode::OK);

    let conversion = client
        .post(format!("{base}/conversion"))
        .header("content-type", "text/markdown")
        .header("accept", "application/vnd.docspec.blocknote+json")
        .body("# Hello World")
        .send()
        .expect("conversion request");
    assert_eq!(conversion.status(), reqwest::StatusCode::OK);

    let response = client
        .get(format!("{base}/metrics"))
        .send()
        .expect("metrics request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .expect("content-type header")
            .to_str()
            .expect("content-type header value"),
        "text/plain; version=0.0.4; charset=utf-8"
    );

    let body = response.text().expect("metrics body");
    for metric_name in [
        "docspec_http_requests_total",
        "docspec_http_request_duration_seconds",
        "docspec_http_request_body_bytes",
        "docspec_conversions_total",
        "docspec_conversion_duration_seconds",
    ] {
        let help_prefix = format!("# HELP {metric_name} ");
        assert!(
            body.lines().any(|line| line.starts_with(&help_prefix)),
            "missing HELP line for {metric_name} in:\n{body}"
        );
    }
    let body_after_second_scrape = client
        .get(format!("{base}/metrics"))
        .send()
        .expect("second metrics request")
        .text()
        .expect("second metrics body");

    let metrics_in_counter = body_after_second_scrape
        .lines()
        .filter(|line| line.starts_with("docspec_http_requests_total{"))
        .any(|line| line.contains(r#"path="/metrics""#));

    assert!(
        !metrics_in_counter,
        "/metrics appeared in docspec_http_requests_total:\n{body_after_second_scrape}"
    );
}
