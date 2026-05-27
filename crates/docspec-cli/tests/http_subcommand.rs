//! Integration tests for the `docspec http` subcommand.

#![allow(clippy::panic, clippy::single_call_fn, clippy::unwrap_used)]
// Reason: Integration tests use unwrap/panic for assertion clarity;
// docspec_cmd is called from multiple tests which is the correct dispatch pattern.

use assert_cmd::Command;

fn docspec_cmd() -> Command {
    let result = Command::cargo_bin("docspec");
    assert!(result.is_ok(), "docspec binary not found");
    result.unwrap_or_else(|_| Command::new(""))
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use predicates::str::contains;

    use super::docspec_cmd;

    #[test]
    fn http_help_shows_all_flags() {
        docspec_cmd()
            .args(["http", "--help"])
            .assert()
            .success()
            .stdout(contains("--host"))
            .stdout(contains("--log-format"))
            .stdout(contains("--log-level"))
            .stdout(contains("--port"));
    }

    #[test]
    fn invalid_port_rejected() {
        docspec_cmd()
            .args(["http", "--port", "99999"])
            .assert()
            .failure()
            .code(2);
    }

    #[test]
    fn port_in_use_exits_nonzero() {
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        docspec_cmd()
            .args(["http", "--port", &port.to_string()])
            .timeout(Duration::from_secs(3))
            .assert()
            .failure();

        drop(listener);
    }

    #[cfg(unix)]
    #[test]
    fn sigint_clean_exit() {
        use std::process::{Command as StdCommand, Stdio};

        let binary = assert_cmd::cargo::cargo_bin("docspec");

        let mut child = StdCommand::new(&binary)
            .args(["http", "--port", "0"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        // Give the server time to bind and start accepting before sending SIGINT.
        std::thread::sleep(Duration::from_millis(500));

        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(i32::try_from(child.id()).unwrap()),
            nix::sys::signal::Signal::SIGINT,
        )
        .unwrap();

        let status = child.wait().unwrap();
        assert_eq!(status.code(), Some(0), "expected exit code 0 after SIGINT");
    }
}
