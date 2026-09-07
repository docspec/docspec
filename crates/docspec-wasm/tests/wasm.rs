//! Tests.

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "markdown-reader", feature = "blocknote-writer"))]
    mod legacy {
        use docspec_test_utils::assert_json_eq;
        use docspec_wasm::convert_markdown_to_blocknote;
        use serde_json::json;

        #[test]
        fn heading_and_paragraph() {
            let result = convert_markdown_to_blocknote("# Hello\n\nWorld");
            assert!(result.is_ok(), "conversion failed: {result:?}");
            assert_json_eq(
                &result.unwrap_or_default(),
                json!([
                    {
                        "type": "heading",
                        "props": {"level": 1},
                        "content": [{"type": "text", "text": "Hello", "styles": {}}],
                        "children": []
                    },
                    {
                        "type": "paragraph",
                        "content": [{"type": "text", "text": "World", "styles": {}}],
                        "children": []
                    }
                ]),
            );
        }

        #[test]
        fn empty_input() {
            let result = convert_markdown_to_blocknote("");
            assert!(result.is_ok(), "empty input should succeed: {result:?}");
            assert_json_eq(&result.unwrap_or_default(), json!([]));
        }

        #[test]
        fn plain_paragraph() {
            let result = convert_markdown_to_blocknote("Just a paragraph");
            assert!(result.is_ok(), "conversion failed: {result:?}");
            assert_json_eq(
                &result.unwrap_or_default(),
                json!([
                    {
                        "type": "paragraph",
                        "content": [{"type": "text", "text": "Just a paragraph", "styles": {}}],
                        "children": []
                    }
                ]),
            );
        }

        #[test]
        fn bold_and_italic_formatting() {
            let result = convert_markdown_to_blocknote("**bold** and *italic*");
            assert!(result.is_ok(), "conversion failed: {result:?}");
            assert_json_eq(
                &result.unwrap_or_default(),
                json!([
                    {
                        "type": "paragraph",
                        "content": [
                            {"type": "text", "text": "bold", "styles": {"bold": true}},
                            {"type": "text", "text": " and ", "styles": {}},
                            {"type": "text", "text": "italic", "styles": {"italic": true}}
                        ],
                        "children": []
                    }
                ]),
            );
        }
    }

    #[test]
    fn capability_lists_are_sorted_and_match_features() {
        let expected_inputs: Vec<String> = vec![
            #[cfg(feature = "docx-reader")]
            String::from("docx"),
            #[cfg(feature = "html-reader")]
            String::from("html"),
            #[cfg(feature = "markdown-reader")]
            String::from("markdown"),
        ];

        let expected_outputs: Vec<String> = vec![
            #[cfg(feature = "blocknote-writer")]
            String::from("blocknote"),
            #[cfg(feature = "html-writer")]
            String::from("html"),
            #[cfg(feature = "markdown-writer")]
            String::from("markdown"),
            #[cfg(feature = "oxa-writer")]
            String::from("oxa"),
            #[cfg(feature = "pandoc-native-writer")]
            String::from("pandoc-native"),
        ];

        assert_eq!(docspec_wasm::input_formats(), expected_inputs);
        assert_eq!(docspec_wasm::output_formats(), expected_outputs);
    }
}
