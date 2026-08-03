//! Structural JSON assertions for writer tests.
//!
//! Writer tests assert on document structure, not on byte-level formatting, so
//! the expectation is written as a readable [`serde_json::json!`] literal (or
//! loaded from a fixture) and the writer output is parsed before comparison.

use serde_json::Value;

/// Asserts that `actual` parses as JSON equal to the `expected` value.
///
/// Key order and whitespace in `actual` are irrelevant: both sides are compared
/// as parsed JSON. Pass expectations as [`serde_json::json!`] literals — a
/// `&str` expectation is a JSON *string* value, not JSON text; use
/// [`assert_json_text_eq`] to compare against JSON text.
///
/// ```
/// use docspec_test_utils::assert_json_eq;
/// use serde_json::json;
///
/// assert_json_eq(
///     r#"[{"type":"paragraph","content":[],"children":[]}]"#,
///     json!([{"type": "paragraph", "content": [], "children": []}]),
/// );
/// ```
///
/// # Panics
///
/// Panics if `actual` is not valid JSON, or if it differs from `expected`. The
/// mismatch is reported as pretty-printed JSON on both sides.
#[inline]
#[track_caller]
pub fn assert_json_eq<E>(actual: &str, expected: E)
where
    E: Into<Value>,
{
    let actual_text = pretty(&parse(actual, "writer output"));
    let expected_text = pretty(&expected.into());
    assert!(
        actual_text == expected_text,
        "JSON mismatch\n--- actual ---\n{actual_text}\n--- expected ---\n{expected_text}"
    );
}

/// Asserts that `actual` and `expected` are equal JSON documents.
///
/// The counterpart of [`assert_json_eq`] for expectations that live in a
/// fixture file rather than in the test body.
///
/// # Panics
///
/// Panics if either side is not valid JSON, or if the two differ.
#[inline]
#[track_caller]
pub fn assert_json_text_eq(actual: &str, expected: &str) {
    assert_json_eq(actual, parse(expected, "expected JSON"));
}

/// Parses `text`, panicking with `label` when it is not valid JSON.
#[track_caller]
fn parse(text: &str, label: &str) -> Value {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        panic!("{label} must be valid JSON, got: {text}");
    };
    value
}

/// Renders a JSON value for comparison and failure output.
fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{assert_json_eq, assert_json_text_eq};

    #[test]
    fn matching_document_passes_regardless_of_key_order() {
        assert_json_eq(
            r#"{"type":"Text","value":"hello"}"#,
            json!({"value": "hello", "type": "Text"}),
        );
    }

    #[test]
    #[should_panic(expected = "JSON mismatch")]
    fn differing_document_panics_with_both_sides() {
        assert_json_eq(r#"{"value":"hello"}"#, json!({"value": "goodbye"}));
    }

    #[test]
    #[should_panic(expected = "writer output must be valid JSON, got: {oops")]
    fn invalid_actual_json_panics_with_the_output() {
        assert_json_eq("{oops", json!({}));
    }

    #[test]
    fn matching_text_documents_pass() {
        assert_json_text_eq(r#"[{"a":1}]"#, "[\n  {\n    \"a\": 1\n  }\n]");
    }

    #[test]
    #[should_panic(expected = "expected JSON must be valid JSON, got: nope")]
    fn invalid_expected_text_panics() {
        assert_json_text_eq("[]", "nope");
    }
}
