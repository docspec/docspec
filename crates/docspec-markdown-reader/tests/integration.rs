//! Markdown to `BlockNote` JSON pipeline integration tests.

use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::{EventSink as _, EventSource as _};
use docspec_markdown_reader::MarkdownReader;

fn run_pipeline(markdown: &str) -> String {
    let mut reader = MarkdownReader::new(markdown);
    let mut buf = Vec::<u8>::new();
    let mut writer = BlockNoteWriter::new(&mut buf);

    let mut next = reader.next_event();
    while let Ok(Some(event)) = next {
        let handle_result = writer.handle_event(event);
        assert!(
            handle_result.is_ok(),
            "handle_event failed: {:?}",
            handle_result.err()
        );
        next = reader.next_event();
    }
    assert!(next.is_ok(), "next_event failed: {:?}", next.err());

    let finish_result = writer.finish();
    assert!(
        finish_result.is_ok(),
        "finish failed: {:?}",
        finish_result.err()
    );

    let string_result = String::from_utf8(buf);
    assert!(string_result.is_ok(), "invalid UTF-8 output");
    string_result.unwrap_or_default()
}

fn assert_json_eq(actual: &str, expected: &str) {
    let actual_parsed = serde_json::from_str::<serde_json::Value>(actual);
    assert!(actual_parsed.is_ok(), "actual is not valid JSON: {actual}");
    let expected_parsed = serde_json::from_str::<serde_json::Value>(expected);
    assert!(
        expected_parsed.is_ok(),
        "expected fixture is not valid JSON"
    );
    assert_eq!(
        actual_parsed.unwrap_or_default(),
        expected_parsed.unwrap_or_default(),
        "JSON mismatch\nActual:   {actual}\nExpected: {expected}"
    );
}

#[cfg(test)]
mod tests {
    use super::{assert_json_eq, run_pipeline};

    #[test]
    fn pipeline_empty() {
        let markdown = include_str!("../../../tests/fixtures/markdown/empty.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/empty.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_heading_levels() {
        let markdown = include_str!("../../../tests/fixtures/markdown/heading_levels.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/heading_levels.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_paragraphs() {
        let markdown = include_str!("../../../tests/fixtures/markdown/paragraphs.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/paragraphs.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_images() {
        let markdown = include_str!("../../../tests/fixtures/markdown/images.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/images.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_mixed() {
        let markdown = include_str!("../../../tests/fixtures/markdown/mixed.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/mixed.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_nested_content() {
        let markdown = include_str!("../../../tests/fixtures/markdown/nested_content.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/nested_content.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_inline_images() {
        let markdown = include_str!("../../../tests/fixtures/markdown/inline_images.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/inline_images.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }
}
