//! Integration tests for the `docspec` CLI binary.

use std::io::Write as _;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;
use predicates::str::contains;
use tempfile::NamedTempFile;

fn docspec_cmd() -> Command {
    let result = Command::cargo_bin("docspec");
    assert!(
        result.is_ok(),
        "docspec binary should be built for integration tests: {:?}",
        result.as_ref().err()
    );
    result.unwrap_or_else(|_| std::process::abort())
}

fn markdown_tempfile(content: &[u8], suffix: &str) -> NamedTempFile {
    let result = tempfile::Builder::new().suffix(suffix).tempfile();
    assert!(
        result.is_ok(),
        "test tempfile should be created: {:?}",
        result.as_ref().err()
    );
    let mut file = result.unwrap_or_else(|_| std::process::abort());

    let write_result = file.write_all(content);
    assert!(
        write_result.is_ok(),
        "test tempfile content should be written: {:?}",
        write_result.err()
    );
    file
}

fn empty_tempfile(suffix: &str) -> NamedTempFile {
    let result = tempfile::Builder::new().suffix(suffix).tempfile();
    assert!(
        result.is_ok(),
        "test tempfile should be created: {:?}",
        result.as_ref().err()
    );
    result.unwrap_or_else(|_| std::process::abort())
}

fn read_output(path: &std::path::Path) -> String {
    let result = std::fs::read_to_string(path);
    assert!(
        result.is_ok(),
        "CLI output file should be readable: {:?}",
        result.as_ref().err()
    );
    result.unwrap_or_else(|_| std::process::abort())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_detect_from_extension() {
        let input = markdown_tempfile(b"# Auto Detected\n", ".md");
        let output = empty_tempfile(".json");
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        assert_eq!(
            read_output(&output_path),
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Auto Detected","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn color_always_flag_enables_ansi() {
        docspec_cmd()
            .args([
                "--color",
                "always",
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
        let input = markdown_tempfile(b"# Hello World\n\nSome paragraph text.\n", ".md");
        let output = empty_tempfile(".json");
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        assert_eq!(
            read_output(&output_path),
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Hello World","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Some paragraph text.","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn convert_stdin_to_stdout() {
        docspec_cmd()
            .args(["--from", "markdown", "--to", "blocknote"])
            .write_stdin("# Hello\n")
            .assert()
            .success()
            .stdout(r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#);
    }

    #[test]
    fn dash_means_stdin() {
        docspec_cmd()
            .args(["-", "--from", "markdown", "--to", "blocknote"])
            .write_stdin("# Dash Input\n")
            .assert()
            .success()
            .stdout(r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Dash Input","styles":{}}],"children":[]}]"#);
    }

    #[test]
    fn empty_markdown_file() {
        let input = empty_tempfile(".md");
        let output = empty_tempfile(".json");
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        assert_eq!(read_output(&output_path), "[]");
    }

    #[test]
    fn explicit_format_flags() {
        let input = markdown_tempfile(b"# Explicit\n", ".txt");
        let output = empty_tempfile(".txt");
        let output_path = output.path().to_path_buf();

        docspec_cmd()
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

        assert_eq!(
            read_output(&output_path),
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Explicit","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn heading_levels_conversion() {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/markdown/heading_levels.md"
        );

        docspec_cmd()
            .args([fixture, "--to", "blocknote"])
            .assert()
            .success()
            .stdout(r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Heading Level 1","styles":{}}],"children":[]},{"type":"heading","props":{"level":2,"textAlignment":"left"},"content":[{"type":"text","text":"Heading Level 2","styles":{}}],"children":[]},{"type":"heading","props":{"level":3,"textAlignment":"left"},"content":[{"type":"text","text":"Heading Level 3","styles":{}}],"children":[]},{"type":"heading","props":{"level":4,"textAlignment":"left"},"content":[{"type":"text","text":"Heading Level 4","styles":{}}],"children":[]},{"type":"heading","props":{"level":5,"textAlignment":"left"},"content":[{"type":"text","text":"Heading Level 5","styles":{}}],"children":[]},{"type":"heading","props":{"level":6,"textAlignment":"left"},"content":[{"type":"text","text":"Heading Level 6","styles":{}}],"children":[]}]"#);
    }

    #[test]
    fn help_flag() {
        docspec_cmd()
            .arg("--help")
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
            .arg("--invalid-flag-xyz")
            .assert()
            .failure()
            .code(2);
    }

    #[test]
    fn missing_input_file_exits_1() {
        docspec_cmd()
            .args([
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
            .args([fixture, "--to", "blocknote"])
            .assert()
            .success()
            .stdout(r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Single paragraph with plain text.","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Multiple paragraphs. This is the second paragraph.","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Paragraph with ","styles":{}},{"type":"text","text":"bold text","styles":{"bold":true}},{"type":"text","text":" in the middle.","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Paragraph with ","styles":{}},{"type":"text","text":"italic text","styles":{"italic":true}},{"type":"text","text":" in the middle.","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Paragraph with ","styles":{}},{"type":"text","text":"bold and italic text","styles":{"bold":true,"italic":true}},{"type":"text","text":" combined.","styles":{}}],"children":[]}]"#);
    }

    #[test]
    fn same_input_output_exits_1() {
        let input = markdown_tempfile(b"# Test\n", ".md");
        let path_str = input.path().to_str().unwrap_or("");

        docspec_cmd()
            .args([path_str, "-o", path_str])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("error:"));
    }

    #[test]
    fn unknown_extension_requires_from_flag() {
        let input = markdown_tempfile(b"# Hello\n", ".xyz");

        docspec_cmd()
            .arg(input.path())
            .args(["-t", "blocknote"])
            .assert()
            .failure()
            .stderr(contains("error:"));
    }

    #[test]
    fn unsupported_input_format_exits_1() {
        let input = markdown_tempfile(b"[]", ".json");

        docspec_cmd()
            .arg(input.path())
            .args(["-t", "blocknote"])
            .assert()
            .failure()
            .code(1)
            .stderr(contains("error:"));
    }

    #[test]
    fn html_file_to_blocknote_via_extension_autodetect() {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/html/paragraphs.html"
        );
        let expected = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/blocknote/paragraphs_from_html.json"
        ));
        let expected_value: serde_json::Value =
            serde_json::from_str(expected).unwrap_or_else(|_| std::process::abort());

        let assert = docspec_cmd()
            .args([fixture, "--to", "blocknote"])
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone())
            .unwrap_or_else(|_| std::process::abort());
        let actual_value: serde_json::Value =
            serde_json::from_str(&stdout).unwrap_or_else(|_| std::process::abort());
        assert_eq!(actual_value, expected_value);
    }

    #[test]
    fn html_explicit_from_flag_via_stdin() {
        docspec_cmd()
            .args(["--from", "html", "--to", "blocknote", "-"])
            .write_stdin("<p>hello world</p>")
            .assert()
            .success()
            .stdout(r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"hello world","styles":{}}],"children":[]}]"#);
    }

    #[test]
    fn empty_html_file_produces_empty_blocknote_array() {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/html/empty.html"
        );
        docspec_cmd()
            .args([fixture, "--to", "blocknote"])
            .assert()
            .success()
            .stdout("[]");
    }

    #[test]
    fn clap_rejects_blocknote_input_with_exit_code_2() {
        docspec_cmd()
            .args(["--from", "blocknote", "x.md"])
            .assert()
            .failure()
            .code(2)
            .stderr(contains("invalid value 'blocknote'"));
    }

    #[test]
    fn clap_rejects_markdown_output_with_exit_code_2() {
        docspec_cmd()
            .args(["--to", "markdown", "x.md"])
            .assert()
            .failure()
            .code(2)
            .stderr(contains("invalid value 'markdown'"));
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
