//! Integration tests for the `docspec` CLI binary.

use std::io::Write as _;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;
use predicates::str::contains;

fn docspec_cmd() -> Command {
    let result = Command::cargo_bin("docspec");
    assert!(result.is_ok(), "docspec binary not found");
    result.unwrap_or_else(|_| Command::new(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_detect_from_extension() {
        let input_result = tempfile::Builder::new().suffix(".md").tempfile();
        assert!(input_result.is_ok(), "failed to create input tempfile");
        let Ok(mut input) = input_result else { return };
        let write_result = input.write_all(b"# Auto Detected\n");
        assert!(write_result.is_ok(), "failed to write to tempfile");

        let output_result = tempfile::Builder::new().suffix(".json").tempfile();
        assert!(output_result.is_ok(), "failed to create output tempfile");
        let Ok(output) = output_result else { return };
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        let content = std::fs::read_to_string(&output_path).unwrap_or_default();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
        assert!(parsed.is_array(), "expected JSON array, got: {content}");
    }

    #[test]
    fn color_always_flag_enables_ansi() {
        docspec_cmd()
            .args([
                "--color",
                "always",
                "convert",
                "/tmp/nonexistent-docspec-test-file-xyz.md",
                "-t",
                "blocknote",
            ])
            .assert()
            .failure()
            .stderr(contains("\x1b["));
    }

    #[test]
    fn color_never_flag_disables_ansi() {
        docspec_cmd()
            .args([
                "--color",
                "never",
                "convert",
                "/tmp/nonexistent-docspec-test-file-xyz.md",
                "-t",
                "blocknote",
            ])
            .assert()
            .failure()
            .stderr(contains("\x1b[").not());
    }

    #[test]
    fn convert_markdown_file_to_json_file() {
        let input_result = tempfile::Builder::new().suffix(".md").tempfile();
        assert!(input_result.is_ok(), "failed to create input tempfile");
        let Ok(mut input) = input_result else { return };
        let write_result = input.write_all(b"# Hello World\n\nSome paragraph text.\n");
        assert!(write_result.is_ok(), "failed to write to tempfile");

        let output_result = tempfile::Builder::new().suffix(".json").tempfile();
        assert!(output_result.is_ok(), "failed to create output tempfile");
        let Ok(output) = output_result else { return };
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        let content = std::fs::read_to_string(&output_path).unwrap_or_default();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
        assert!(parsed.is_array(), "expected JSON array, got: {content}");
        let text = parsed.to_string();
        assert!(
            text.contains("heading") || text.contains("paragraph"),
            "expected heading or paragraph in output"
        );
    }

    #[test]
    fn convert_stdin_to_stdout() {
        docspec_cmd()
            .args(["convert", "--from", "markdown", "--to", "blocknote"])
            .write_stdin("# Hello\n")
            .assert()
            .success()
            .stdout(contains("heading"));
    }

    #[test]
    fn dash_means_stdin() {
        docspec_cmd()
            .args(["convert", "-", "--from", "markdown", "--to", "blocknote"])
            .write_stdin("# Dash Input\n")
            .assert()
            .success()
            .stdout(contains("heading"));
    }

    #[test]
    fn empty_markdown_file() {
        let input_result = tempfile::Builder::new().suffix(".md").tempfile();
        assert!(input_result.is_ok(), "failed to create input tempfile");
        let Ok(input) = input_result else { return };

        let output_result = tempfile::Builder::new().suffix(".json").tempfile();
        assert!(output_result.is_ok(), "failed to create output tempfile");
        let Ok(output) = output_result else { return };
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        let content = std::fs::read_to_string(&output_path).unwrap_or_default();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
        assert!(parsed.is_array(), "expected JSON array, got: {content}");
    }

    #[test]
    fn explicit_format_flags() {
        let input_result = tempfile::Builder::new().suffix(".txt").tempfile();
        assert!(input_result.is_ok(), "failed to create input tempfile");
        let Ok(mut input) = input_result else { return };
        let write_result = input.write_all(b"# Explicit\n");
        assert!(write_result.is_ok(), "failed to write to tempfile");

        let output_result = tempfile::Builder::new().suffix(".txt").tempfile();
        assert!(output_result.is_ok(), "failed to create output tempfile");
        let Ok(output) = output_result else { return };
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args([
                "--from",
                "markdown",
                "--to",
                "blocknote",
                "-o",
                output_path.to_str().unwrap_or(""),
            ])
            .assert()
            .success();

        let content = std::fs::read_to_string(&output_path).unwrap_or_default();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
        assert!(parsed.is_array(), "expected JSON array, got: {content}");
    }

    #[test]
    fn heading_levels_conversion() {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/markdown/heading_levels.md"
        );

        docspec_cmd()
            .args(["convert", fixture, "--to", "blocknote"])
            .assert()
            .success()
            .stdout(contains("heading"));
    }

    #[test]
    fn help_flag() {
        docspec_cmd()
            .args(["convert", "--help"])
            .assert()
            .success()
            .stdout(contains("--from"))
            .stdout(contains("--to"))
            .stdout(contains("--output"))
            .stdout(contains("--color"));
    }

    #[test]
    fn invalid_arguments_exits_2() {
        docspec_cmd()
            .args(["convert", "--invalid-flag-xyz"])
            .assert()
            .failure()
            .code(2);
    }

    #[test]
    fn missing_input_file_exits_1() {
        docspec_cmd()
            .args([
                "convert",
                "/tmp/nonexistent-docspec-test-file-xyz.md",
                "-t",
                "blocknote",
            ])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("error:"));
    }

    #[test]
    fn no_color_env_disables_ansi() {
        docspec_cmd()
            .env("NO_COLOR", "1")
            .args([
                "convert",
                "/tmp/nonexistent-docspec-test-file-xyz.md",
                "-t",
                "blocknote",
            ])
            .assert()
            .failure()
            .stderr(contains("\x1b[").not());
    }

    #[test]
    fn paragraph_text_conversion() {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/markdown/paragraphs.md"
        );

        docspec_cmd()
            .args(["convert", fixture, "--to", "blocknote"])
            .assert()
            .success()
            .stdout(contains("paragraph"));
    }

    #[test]
    fn same_input_output_exits_1() {
        let input_result = tempfile::Builder::new().suffix(".md").tempfile();
        assert!(input_result.is_ok(), "failed to create tempfile");
        let Ok(mut input) = input_result else { return };
        let write_result = input.write_all(b"# Test\n");
        assert!(write_result.is_ok(), "failed to write to tempfile");
        let path_str = input.path().to_str().unwrap_or("");

        docspec_cmd()
            .args(["convert", path_str, "-o", path_str])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("error:"));
    }

    #[test]
    fn unknown_extension_requires_from_flag() {
        let input_result = tempfile::Builder::new().suffix(".xyz").tempfile();
        assert!(input_result.is_ok(), "failed to create tempfile");
        let Ok(mut input) = input_result else { return };
        let write_result = input.write_all(b"# Hello\n");
        assert!(write_result.is_ok(), "failed to write to tempfile");

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["-t", "blocknote"])
            .assert()
            .failure()
            .stderr(contains("error:"));
    }

    #[test]
    fn unsupported_input_format_exits_1() {
        let input_result = tempfile::Builder::new().suffix(".json").tempfile();
        assert!(input_result.is_ok(), "failed to create tempfile");
        let Ok(mut input) = input_result else { return };
        let write_result = input.write_all(b"[]");
        assert!(write_result.is_ok(), "failed to write to tempfile");

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["-t", "blocknote"])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("not yet implemented"));
    }

    #[test]
    fn version_flag() {
        docspec_cmd()
            .arg("--version")
            .assert()
            .success()
            .stdout(contains("0.1.0"));
    }
}
