//! Tests.

#[cfg(test)]
mod tests {
    use docspec_wasm::convert_markdown_to_blocknote;

    #[test]
    fn heading_and_paragraph() {
        let result = convert_markdown_to_blocknote("# Hello\n\nWorld");
        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert_eq!(
            result.unwrap_or_default(),
            r#"[{"type":"heading","props":{"level":1,"textAlignment":"left"},"content":[{"type":"text","text":"Hello","styles":{}}],"children":[]},{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"World","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn empty_input() {
        let result = convert_markdown_to_blocknote("");
        assert!(result.is_ok(), "empty input should succeed: {result:?}");
        assert_eq!(result.unwrap_or_default(), "[]");
    }

    #[test]
    fn plain_paragraph() {
        let result = convert_markdown_to_blocknote("Just a paragraph");
        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert_eq!(
            result.unwrap_or_default(),
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"Just a paragraph","styles":{}}],"children":[]}]"#
        );
    }

    #[test]
    fn bold_and_italic_formatting() {
        let result = convert_markdown_to_blocknote("**bold** and *italic*");
        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert_eq!(
            result.unwrap_or_default(),
            r#"[{"type":"paragraph","props":{"textAlignment":"left"},"content":[{"type":"text","text":"bold","styles":{"bold":true}},{"type":"text","text":" and ","styles":{}},{"type":"text","text":"italic","styles":{"italic":true}}],"children":[]}]"#
        );
    }

    #[test]
    fn bom_prefixed_markdown_strips_bom() {
        let result = convert_markdown_to_blocknote("\u{FEFF}# Hello").unwrap();
        // The BOM should be stripped by the facade; heading text should be "Hello"
        assert!(result.contains("Hello"), "Expected 'Hello' in output, got: {result}");
        assert!(!result.contains("\u{FEFF}"), "BOM should not appear in output");
    }
}
