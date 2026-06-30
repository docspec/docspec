//! Markdown to `BlockNote` JSON pipeline integration tests.
#![cfg(all(feature = "markdown", feature = "blocknote-writer"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use docspec::readers::MarkdownReader;
use docspec::writers::BlockNoteWriter;
use docspec::{EventSink as _, EventSource as _, StackTrackingSink};

fn try_run_pipeline(markdown: &str) -> Result<String, String> {
    let mut reader = MarkdownReader::from_str(markdown);
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
    let actual_parsed =
        serde_json::from_str::<serde_json::Value>(actual).expect("actual is valid JSON");
    let expected_parsed =
        serde_json::from_str::<serde_json::Value>(expected).expect("fixture is valid JSON");
    assert_eq!(
        actual_parsed, expected_parsed,
        "JSON mismatch\nActual:   {actual}\nExpected: {expected}"
    );
}

#[cfg(test)]
mod tests {
    use super::{assert_json_eq, run_pipeline};

    fn load_blocknote_fixture(name: &str) -> serde_json::Value {
        let path = format!(
            "{}/../../tests/fixtures/blocknote/{}",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        let content = std::fs::read_to_string(&path).expect("fixture should be readable");
        serde_json::from_str(&content).expect("fixture should be valid JSON")
    }

    fn assert_fixture_eq(actual: &str, fixture: &str) {
        let actual_json: serde_json::Value =
            serde_json::from_str(actual).expect("actual output must be valid JSON");
        assert_eq!(actual_json, load_blocknote_fixture(fixture));
    }

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
        let markdown = "5) I2\n   > text\n   > - [f]\n";
        let actual = run_pipeline(markdown);
        assert_json_eq(
            &actual,
            r#"[{"type":"numberedListItem","props":{"start":5},"content":[{"type":"text","text":"I2","styles":{}}],"children":[{"type":"quote","content":[{"type":"text","text":"text","styles":{}}],"children":[{"type":"bulletListItem","content":[{"type":"text","text":"[","styles":{}},{"type":"text","text":"f","styles":{}},{"type":"text","text":"]","styles":{}}],"children":[]}]}]}]"#,
        );
    }

    #[test]
    fn integration_simple_bullet_list_matches_fixture() {
        let json = run_pipeline("- a\n- b\n- c");
        assert_fixture_eq(&json, "lists_simple_bullet.json");
    }

    #[test]
    fn integration_simple_numbered_list_matches_fixture() {
        let json = run_pipeline("1. one\n2. two\n3. three");
        assert_fixture_eq(&json, "lists_simple_numbered.json");
    }

    #[test]
    fn integration_nested_bullets_matches_fixture() {
        let json = run_pipeline("- a\n  - b\n  - c\n- d");
        assert_fixture_eq(&json, "lists_nested_bullets.json");
    }

    #[test]
    fn integration_mixed_types_matches_fixture() {
        let json = run_pipeline("- bullet\n1. numbered\n- another bullet");
        assert_fixture_eq(&json, "lists_mixed_types.json");
    }

    #[test]
    fn integration_multi_paragraph_item_matches_fixture() {
        let json = run_pipeline("- first para\n\n  second para\n- next item");
        assert_fixture_eq(&json, "lists_multi_paragraph_item.json");
    }
}
