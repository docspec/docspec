//! End-to-end shutdown test.
//!
//! Installs a SIGTERM handler in the test process before spawning the server so the OS
//! cannot default-kill the test between our `kill()` call and the server installing its
//! own handler. The test then retry-sends SIGTERM until the server task finishes,
//! eliminating the race over signal-handler installation timing.
//!
//! Lives in its own integration-test binary so process-wide signals cannot disturb other
//! tests in the workspace.

#![allow(clippy::tests_outside_test_module, clippy::unwrap_used)]
// Reason: Integration tests in `tests/` are always outside `#[cfg(test)]`; unwrap is fine for assertion clarity.

#[cfg(unix)]
#[tokio::test]
async fn server_shuts_down_on_sigterm() {
    use core::time::Duration;

    let mut sigterm =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();

    let server_task = tokio::spawn(async { docspec_http::serve("127.0.0.1", 0).await });

    let pid = nix::unistd::Pid::from_raw(i32::try_from(std::process::id()).unwrap());
    let timeout = Duration::from_secs(5);
    let start = std::time::Instant::now();
    while !server_task.is_finished() && start.elapsed() < timeout {
        nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM).unwrap();
        match tokio::time::timeout(Duration::from_millis(50), sigterm.recv()).await {
            Ok(_) | Err(_) => {}
        }
    }

    assert!(
        server_task.is_finished(),
        "server did not shut down within 5s of SIGTERM"
    );
}
