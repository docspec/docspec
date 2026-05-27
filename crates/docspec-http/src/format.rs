//! MIME type constants and content-type parsing helpers.

/// MIME type for Markdown input.
pub const INPUT_MARKDOWN: &str = "text/markdown";

/// MIME type for `BlockNote` JSON output.
pub const OUTPUT_BLOCKNOTE: &str = "application/vnd.docspec.blocknote+json";

/// MIME type for RFC 7807 problem+json error responses.
pub const ERROR_PROBLEM_JSON: &str = "application/problem+json";

/// Returns `true` if the given Content-Type value indicates Markdown.
///
/// Accepts `text/markdown` with optional parameters (e.g., `; charset=utf-8`, `; variant=CommonMark`).
/// Matching is case-insensitive.
///
/// # Examples
///
/// ```
/// use docspec_http::format::is_markdown;
/// assert!(is_markdown("text/markdown"));
/// assert!(is_markdown("text/markdown; charset=utf-8"));
/// assert!(!is_markdown("text/plain"));
/// ```
#[inline]
#[must_use]
pub fn is_markdown(content_type: &str) -> bool {
    let base = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim();
    base.eq_ignore_ascii_case("text/markdown")
}

/// Returns `true` if the given Accept header value accepts `BlockNote` JSON output.
///
/// Applies RFC 7231 §5.3.2 media-range precedence: the most specific matching
/// range determines acceptability via its q-value, with the precedence order
/// exact (`application/vnd.docspec.blocknote+json`) > subtype wildcard
/// (`application/*`) > full wildcard (`*/*`). A more-specific `q=0` range
/// overrides a less-specific `q>0` range, so
/// `application/vnd.docspec.blocknote+json;q=0, */*` is rejected. Returns
/// `true` when the header is absent (RFC 7231 §5.3.2).
///
/// # Examples
///
/// ```
/// use docspec_http::format::accepts_blocknote;
/// assert!(accepts_blocknote(None));
/// assert!(accepts_blocknote(Some("*/*")));
/// assert!(!accepts_blocknote(Some("text/html")));
/// assert!(!accepts_blocknote(Some(
///     "application/vnd.docspec.blocknote+json;q=0, */*"
/// )));
/// ```
#[inline]
#[must_use]
pub fn accepts_blocknote(accept: Option<&str>) -> bool {
    let Some(accept_str) = accept else {
        return true;
    };

    let mut best_specificity: u8 = 0;
    let mut best_q: f32 = 0.0;

    for raw in accept_str.split(',') {
        let mut parts = raw.trim().split(';');
        // Reason: `str::split(';')` always yields at least one item, so `next()` cannot return None here.
        let media = parts.next().unwrap_or("").trim();
        let Some((raw_type, raw_subtype)) = media.split_once('/') else {
            continue;
        };
        let type_t = raw_type.trim();
        let subtype_t = raw_subtype.trim();

        let specificity: u8 = if type_t.eq_ignore_ascii_case("application")
            && subtype_t.eq_ignore_ascii_case("vnd.docspec.blocknote+json")
        {
            3
        } else if type_t.eq_ignore_ascii_case("application") && subtype_t == "*" {
            2
        } else if type_t == "*" && subtype_t == "*" {
            1
        } else {
            continue;
        };

        // Reason: RFC 7231 §5.3.1 defaults missing q to 1.0; invalid q values
        // are ignored (treated as absent) to match permissive proxy behavior.
        let mut q: f32 = 1.0;
        for raw_param in parts {
            let trimmed = raw_param.trim();
            if let Some(q_str) = trimmed
                .strip_prefix("q=")
                .or_else(|| trimmed.strip_prefix("Q="))
            {
                if let Ok(parsed) = q_str.parse::<f32>() {
                    q = parsed.clamp(0.0, 1.0);
                }
            }
        }

        if specificity > best_specificity {
            best_specificity = specificity;
            best_q = q;
        }
    }

    best_specificity > 0 && best_q > 0.0
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    // Reason: Test code may use unwrap for assertion clarity.

    use super::*;

    #[test]
    fn is_markdown_plain() {
        assert!(is_markdown("text/markdown"));
    }

    #[test]
    fn is_markdown_with_charset() {
        assert!(is_markdown("text/markdown; charset=utf-8"));
    }

    #[test]
    fn is_markdown_case_insensitive() {
        assert!(is_markdown("TEXT/MARKDOWN"));
        assert!(is_markdown("Text/Markdown"));
    }

    #[test]
    fn is_markdown_rejects_plain_text() {
        assert!(!is_markdown("text/plain"));
    }

    #[test]
    fn is_markdown_rejects_json() {
        assert!(!is_markdown("application/json"));
    }

    #[test]
    fn accepts_blocknote_none() {
        assert!(accepts_blocknote(None));
    }

    #[test]
    fn accepts_blocknote_wildcard() {
        assert!(accepts_blocknote(Some("*/*")));
    }

    #[test]
    fn accepts_blocknote_rejects_json() {
        assert!(!accepts_blocknote(Some("application/json")));
    }

    #[test]
    fn accepts_blocknote_rejects_empty_media_range() {
        assert!(!accepts_blocknote(Some("")));
    }

    #[test]
    fn accepts_blocknote_rejects_missing_subtype() {
        assert!(!accepts_blocknote(Some("application")));
    }

    #[test]
    fn accepts_blocknote_exact() {
        assert!(accepts_blocknote(Some(
            "application/vnd.docspec.blocknote+json"
        )));
    }

    #[test]
    fn accepts_blocknote_with_quality() {
        assert!(accepts_blocknote(Some(
            "application/vnd.docspec.blocknote+json; q=0.9, text/html"
        )));
    }

    #[test]
    fn accepts_blocknote_rejects_jsonp_suffix() {
        assert!(!accepts_blocknote(Some(
            "application/vnd.docspec.blocknote+jsonp"
        )));
    }

    #[test]
    fn accepts_blocknote_rejects_exact_q_zero() {
        assert!(!accepts_blocknote(Some(
            "application/vnd.docspec.blocknote+json;q=0"
        )));
    }

    #[test]
    fn accepts_blocknote_rejects_wildcard_q_zero() {
        assert!(!accepts_blocknote(Some("*/*;q=0")));
    }

    #[test]
    fn accepts_blocknote_accepts_second_media_range() {
        assert!(accepts_blocknote(Some(
            "application/json, application/vnd.docspec.blocknote+json"
        )));
    }

    #[test]
    fn accepts_blocknote_accepts_application_wildcard_with_quality() {
        assert!(accepts_blocknote(Some(
            "text/html;q=0.9, application/*;q=0.8"
        )));
    }

    #[test]
    fn accepts_blocknote_honors_multiple_quality_ranges() {
        assert!(accepts_blocknote(Some(
            "text/html;q=0.9, application/json;q=0, application/vnd.docspec.blocknote+json;q=0.2"
        )));
    }

    #[test]
    fn accepts_blocknote_ignores_invalid_quality_value() {
        assert!(accepts_blocknote(Some(
            "application/vnd.docspec.blocknote+json;q=maybe"
        )));
    }

    #[test]
    fn accepts_blocknote_exact_q_zero_overrides_wildcard_q_one() {
        assert!(!accepts_blocknote(Some(
            "application/vnd.docspec.blocknote+json;q=0, */*;q=1"
        )));
    }

    #[test]
    fn accepts_blocknote_exact_q_one_overrides_wildcard_q_zero() {
        assert!(accepts_blocknote(Some(
            "application/vnd.docspec.blocknote+json;q=1, */*;q=0"
        )));
    }

    #[test]
    fn accepts_blocknote_subtype_wildcard_q_zero_overrides_full_wildcard_q_one() {
        assert!(!accepts_blocknote(Some("application/*;q=0, */*;q=1")));
    }
}
