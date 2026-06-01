//! Markdown to `BlockNote` JSON pipeline integration tests.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use docspec_blocknote_writer::BlockNoteWriter;
use docspec_core::{EventSink as _, EventSource as _, StackTrackingSink};
use docspec_markdown_reader::MarkdownReader;

fn try_run_pipeline(markdown: &str) -> Result<String, String> {
    let mut reader = MarkdownReader::new(markdown);
    let mut buf = Vec::<u8>::new();
    let mut writer = StackTrackingSink::new(BlockNoteWriter::new(&mut buf));

    while let Some(event) = reader.next_event().map_err(|e| format!("{e:?}"))? {
        writer.handle_event(event).map_err(|e| format!("{e:?}"))?;
    }
    writer.finish().map_err(|e| format!("{e:?}"))?;

    String::from_utf8(buf).map_err(|e| format!("{e}"))
}

fn run_pipeline(markdown: &str) -> String {
    try_run_pipeline(markdown).expect("pipeline failed")
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

    #[test]
    fn pipeline_blockquote() {
        let markdown = include_str!("../../../tests/fixtures/markdown/blockquote.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/blockquote.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_blockquote_multiline() {
        let markdown = include_str!("../../../tests/fixtures/markdown/blockquote_multiline.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/blockquote_multiline.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_text_formatting() {
        let markdown = include_str!("../../../tests/fixtures/markdown/text_formatting.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/text_formatting.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_tables() {
        let markdown = include_str!("../../../tests/fixtures/markdown/tables.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/tables.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_lists() {
        let markdown = include_str!("../../../tests/fixtures/markdown/lists.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/lists.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_links() {
        let markdown = include_str!("../../../tests/fixtures/markdown/links.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/links.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_soft_break() {
        let markdown = include_str!("../../../tests/fixtures/markdown/soft_break.md");
        let expected = include_str!("../../../tests/fixtures/blocknote/soft_break.json");
        let actual = run_pipeline(markdown);
        assert_json_eq(&actual, expected);
    }

    #[test]
    fn pipeline_list_inside_blockquote_inside_list_item_is_well_formed() {
        // Regression for cmark-gfm/test.md edge case.
        // A list nested inside a blockquote that is itself inside a list item
        // must NOT cause premature emission of the outer list item's End event.
        let markdown = "5) I2\n   > text\n   > - [f]\n";
        let result = super::try_run_pipeline(markdown);
        assert!(
            result.is_ok(),
            "Expected well-formed event stream for list-in-blockquote-in-item; got: {:?}",
            result.err()
        );
    }
}
