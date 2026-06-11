//! Integration tests for the `docspec` CLI binary.

#![allow(
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::unwrap_used
)]

use std::io::Write as _;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;
use predicates::str::contains;
use tempfile::NamedTempFile;

use docspec_test_utils::synth_docx;

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

const SIMPLE_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_detect_from_extension() {
        let input = markdown_tempfile(b"# Auto Detected\n", ".md");
        let output = empty_tempfile(".json");
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        assert_eq!(
            read_output(&output_path),
            concat!(
                r#"[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Auto Detected","styles":{}}],"children":[]}]"#,
                "\n"
            )
        );
    }

    #[test]
    fn color_always_flag_enables_ansi() {
        docspec_cmd()
            .args([
                "convert",
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
                "convert",
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
            .arg("convert")
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        assert_eq!(
            read_output(&output_path),
            concat!(
                r#"[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Hello World","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Some paragraph text.","styles":{}}],"children":[]}]"#,
                "\n"
            )
        );
    }

    #[test]
    fn convert_stdin_to_stdout() {
        docspec_cmd()
            .args(["convert", "--from", "markdown", "--to", "blocknote"])
            .write_stdin("# Hello\n")
            .assert()
            .success()
            .stdout(concat!(
                r#"[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#,
                "\n"
            ));
    }

    #[test]
    fn dash_means_stdin() {
        docspec_cmd()
            .args(["convert", "-", "--from", "markdown", "--to", "blocknote"])
            .write_stdin("# Dash Input\n")
            .assert()
            .success()
            .stdout(concat!(
                r#"[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Dash Input","styles":{}}],"children":[]}]"#,
                "\n"
            ));
    }

    #[test]
    fn empty_markdown_file() {
        let input = empty_tempfile(".md");
        let output = empty_tempfile(".json");
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        assert_eq!(read_output(&output_path), "[]\n");
    }

    #[test]
    fn explicit_format_flags() {
        let input = markdown_tempfile(b"# Explicit\n", ".txt");
        let output = empty_tempfile(".txt");
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

        assert_eq!(
            read_output(&output_path),
            concat!(
                r#"[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Explicit","styles":{}}],"children":[]}]"#,
                "\n"
            )
        );
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
            .stdout(concat!(
                r#"[{"type":"heading","props":{"level":1},"content":[{"type":"text","text":"Heading Level 1","styles":{}}],"children":[]},{"type":"heading","props":{"level":2},"content":[{"type":"text","text":"Heading Level 2","styles":{}}],"children":[]},{"type":"heading","props":{"level":3},"content":[{"type":"text","text":"Heading Level 3","styles":{}}],"children":[]},{"type":"heading","props":{"level":4},"content":[{"type":"text","text":"Heading Level 4","styles":{}}],"children":[]},{"type":"heading","props":{"level":5},"content":[{"type":"text","text":"Heading Level 5","styles":{}}],"children":[]},{"type":"heading","props":{"level":6},"content":[{"type":"text","text":"Heading Level 6","styles":{}}],"children":[]}]"#,
                "\n"
            ));
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
    fn no_arguments_prints_help() {
        docspec_cmd()
            .assert()
            .failure()
            .stderr(predicates::str::contains("Usage"));
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
            .stdout(concat!(
                r#"[{"type":"paragraph","content":[{"type":"text","text":"Single paragraph with plain text.","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Multiple paragraphs. This is the second paragraph.","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Paragraph with ","styles":{}},{"type":"text","text":"bold text","styles":{"bold":true}},{"type":"text","text":" in the middle.","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Paragraph with ","styles":{}},{"type":"text","text":"italic text","styles":{"italic":true}},{"type":"text","text":" in the middle.","styles":{}}],"children":[]},{"type":"paragraph","content":[{"type":"text","text":"Paragraph with ","styles":{}},{"type":"text","text":"bold and italic text","styles":{"bold":true,"italic":true}},{"type":"text","text":" combined.","styles":{}}],"children":[]}]"#,
                "\n"
            ));
    }

    #[test]
    fn same_input_output_exits_1() {
        let input = markdown_tempfile(b"# Test\n", ".md");
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
        let input = markdown_tempfile(b"# Hello\n", ".xyz");

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
        let input = markdown_tempfile(b"[]", ".json");

        docspec_cmd()
            .arg("convert")
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
            .args(["convert", fixture, "--to", "blocknote"])
            .assert()
            .success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone())
            .unwrap_or_else(|_| std::process::abort());
        assert!(
            stdout.ends_with('\n'),
            "CLI stdout output should end with a newline: {stdout:?}"
        );
        let actual_value: serde_json::Value =
            serde_json::from_str(&stdout).unwrap_or_else(|_| std::process::abort());
        assert_eq!(actual_value, expected_value);
    }

    #[test]
    fn html_to_stdout_defaults_to_blocknote_output() {
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

        let assert = docspec_cmd().arg("convert").arg(fixture).assert().success();

        let stdout = String::from_utf8(assert.get_output().stdout.clone())
            .unwrap_or_else(|_| std::process::abort());
        assert!(
            stdout.ends_with('\n'),
            "CLI stdout output should end with a newline: {stdout:?}"
        );
        let actual_value: serde_json::Value =
            serde_json::from_str(&stdout).unwrap_or_else(|_| std::process::abort());
        assert_eq!(actual_value, expected_value);
    }

    #[test]
    fn html_explicit_from_flag_via_stdin() {
        docspec_cmd()
            .args(["convert", "--from", "html", "--to", "blocknote", "-"])
            .write_stdin("<p>hello world</p>")
            .assert()
            .success()
            .stdout(concat!(
                r#"[{"type":"paragraph","content":[{"type":"text","text":"hello world","styles":{}}],"children":[]}]"#,
                "\n"
            ));
    }

    #[test]
    fn empty_html_file_produces_empty_blocknote_array() {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/html/empty.html"
        );
        docspec_cmd()
            .args(["convert", fixture, "--to", "blocknote"])
            .assert()
            .success()
            .stdout("[]\n");
    }

    #[test]
    fn clap_rejects_blocknote_input_with_exit_code_2() {
        docspec_cmd()
            .args(["convert", "--from", "blocknote", "x.md"])
            .assert()
            .failure()
            .code(2)
            .stderr(contains("invalid value 'blocknote'"));
    }

    #[test]
    fn clap_rejects_markdown_output_with_exit_code_2() {
        docspec_cmd()
            .args(["convert", "--to", "markdown", "x.md"])
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
            .stdout(predicates::str::is_match(r"docspec \d+\.\d+\.\d+").unwrap());
    }

    #[test]
    fn convert_markdown_stdin_to_html_stdout() {
        docspec_cmd()
            .args(["convert", "--from", "markdown", "--to", "html"])
            .write_stdin("Hello world")
            .assert()
            .success()
            .stdout("<html><body><p>Hello world</p></body></html>\n");
    }

    #[test]
    fn markdown_heading_is_dropped_by_paragraph_only_html_writer() {
        docspec_cmd()
            .args(["convert", "--from", "markdown", "--to", "html"])
            .write_stdin("# Dropped\n\nKept paragraph")
            .assert()
            .success()
            .stdout("<html><body><p>Kept paragraph</p></body></html>\n");
    }

    #[test]
    fn convert_markdown_to_html_file_via_extension_autodetect() {
        let input = markdown_tempfile(b"Hello world\n", ".md");
        let output = empty_tempfile(".html");
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        assert_eq!(
            read_output(&output_path),
            "<html><body><p>Hello world</p></body></html>\n"
        );
    }

    #[test]
    fn convert_markdown_stdin_to_oxa_stdout() {
        docspec_cmd()
            .args(["convert", "--from", "markdown", "--to", "oxa"])
            .write_stdin("Hello world")
            .assert()
            .success()
            .stdout(concat!(
                r#"{"type":"Document","children":[{"type":"Paragraph","children":[{"type":"Text","value":"Hello world"}]}]}"#,
                "\n"
            ));
    }

    #[test]
    fn convert_markdown_stdin_to_pandoc_native_stdout() {
        docspec_cmd()
            .args(["convert", "--from", "markdown", "--to", "pandoc-native"])
            .write_stdin("Hello world")
            .assert()
            .success()
            .stdout(concat!(r#"[Para [Str "Hello world"]]"#, "\n"));
    }

    #[test]
    fn json_extension_without_to_flag_still_picks_blocknote() {
        // Regression guard: .json is ambiguous; auto-detection must NOT pick oxa.
        let input = markdown_tempfile(b"Hello\n", ".md");
        let output = empty_tempfile(".json");
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        assert_eq!(
            read_output(&output_path),
            concat!(
                r#"[{"type":"paragraph","content":[{"type":"text","text":"Hello","styles":{}}],"children":[]}]"#,
                "\n"
            )
        );
    }
    #[test]
    fn native_extension_without_to_flag_picks_pandoc_native() {
        let input = markdown_tempfile(b"Hello\n", ".md");
        let output = empty_tempfile(".native");
        let output_path = output.path().to_path_buf();

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["-o", output_path.to_str().unwrap_or("")])
            .assert()
            .success();

        assert_eq!(
            read_output(&output_path),
            concat!(r#"[Para [Str "Hello"]]"#, "\n")
        );
    }

    #[test]
    fn bare_invocation_shows_help_and_exits_nonzero() {
        docspec_cmd()
            .assert()
            .failure()
            .stderr(predicates::str::contains("subcommand").or(predicates::str::contains("Usage")));
    }

    #[test]
    fn convert_docx_file_to_blocknote_stdout() {
        let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#;
        let bytes = synth_docx(SIMPLE_RELS, doc_xml);
        let input = markdown_tempfile(&bytes, ".docx");

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["--to", "blocknote"])
            .assert()
            .success()
            .stdout(contains("paragraph"));
    }

    #[test]
    fn convert_docx_stdin_to_blocknote_stdout() {
        let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#;
        let bytes = synth_docx(SIMPLE_RELS, doc_xml);

        docspec_cmd()
            .args(["convert", "--from", "docx", "--to", "blocknote"])
            .write_stdin(bytes)
            .assert()
            .success()
            .stdout(contains("paragraph"));
    }

    #[test]
    fn docx_stdin_without_from_flag_errors() {
        let doc_xml = r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>hello</w:t></w:r></w:p></w:body></w:document>"#;
        let bytes = synth_docx(SIMPLE_RELS, doc_xml);

        docspec_cmd()
            .args(["convert", "--to", "blocknote"])
            .write_stdin(bytes)
            .assert()
            .failure()
            .stderr(contains("cannot detect input format"));
    }

    #[test]
    fn bom_prefixed_markdown_file_strips_bom() {
        let content = b"\xef\xbb\xbf# Hello";
        let input = markdown_tempfile(content, ".md");

        docspec_cmd()
            .arg("convert")
            .arg(input.path())
            .args(["--to", "blocknote"])
            .assert()
            .success()
            .stdout(contains("Hello"))
            .stdout(predicates::str::contains("\u{FEFF}Hello").not());
    }

    #[test]
    fn bom_prefixed_markdown_stdin_strips_bom() {
        let content = b"\xef\xbb\xbf# Hello";

        docspec_cmd()
            .args(["convert", "--from", "markdown", "--to", "blocknote"])
            .write_stdin(content.as_ref())
            .assert()
            .success()
            .stdout(contains("Hello"))
            .stdout(predicates::str::contains("\u{FEFF}Hello").not());
    }

    #[test]
    fn clap_rejects_unknown_input_format() {
        docspec_cmd()
            .args(["convert", "--from", "pdf", "--to", "blocknote"])
            .assert()
            .failure()
            .stderr(contains("pdf"));
    }
}
