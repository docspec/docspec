//! Relationship-part path derivation, resolution, normalization, and validation.

use docspec_core::Error;

use super::parse_error;

pub(super) fn validate_document_path(document_path: &str) -> docspec_core::Result<String> {
    if document_path.split('/').any(|component| component == "..") {
        return Err(Error::Parse {
            message: format!("rels target contains parent reference: {document_path}"),
            position: None,
        });
    }

    Ok(document_path.to_string())
}

/// Derives the part relationships file path from a part path.
///
/// Per ECMA-376 Part 2 §6.5.2.3:
/// - `"word/document.xml"` → `"word/_rels/document.xml.rels"`
/// - `"foo"` (no directory) → `"_rels/foo.rels"`
pub fn derive_part_rels_path(part_path: &str) -> String {
    part_path.rfind('/').map_or_else(
        || format!("_rels/{part_path}.rels"),
        |slash_pos| {
            let (dir, file_with_slash) = part_path.split_at(slash_pos);
            let file = file_with_slash.strip_prefix('/').unwrap_or_default();
            format!("{dir}/_rels/{file}.rels")
        },
    )
}

/// Resolves a relative target against a base part path.
///
/// Per ECMA-376 Part 2 §6.5.3.4: strip the filename from `base_part`,
/// then join with `target`. Leading `/` in `target` is stripped (absolute paths
/// within the package are treated as relative to the root).
pub fn resolve_relative_target(base_part: &str, target: &str) -> String {
    let target_stripped = target.strip_prefix('/').unwrap_or(target);

    if target.starts_with('/') {
        target_stripped.to_string()
    } else if let Some(slash_pos) = base_part.rfind('/') {
        let (base_dir, _) = base_part.split_at(slash_pos.saturating_add(1));
        format!("{base_dir}{target_stripped}")
    } else {
        target_stripped.to_string()
    }
}

/// Resolves a relative target against `base_part` and canonicalizes `.` / `..`
/// segments per ECMA-376 Part 2 §6.5.3 / RFC 3986 §5.2.
///
/// Word and other DOCX writers routinely emit `Target="../media/image1.png"`
/// or `Target="../customXml/item1.xml"` from `word/_rels/document.xml.rels`;
/// rejecting `..` outright (the old behavior of [`validate_document_path`])
/// breaks reading those files. This helper collapses the segments instead,
/// while still refusing references that escape the package root — a `..`
/// segment that pops past the start of the resolved path returns
/// [`docspec_core::Error::Parse`].
pub(super) fn normalize_relative_target(
    base_part: &str,
    target: &str,
) -> docspec_core::Result<String> {
    let joined = resolve_relative_target(base_part, target);
    let mut normalized: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if normalized.pop().is_none() {
                    return Err(parse_error(format!(
                        "rels target escapes package root: {joined}"
                    )));
                }
            }
            other => normalized.push(other),
        }
    }
    Ok(normalized.join("/"))
}

#[cfg(all(test, coverage))]
mod coverage_tests {
    use super::*;

    #[test]
    fn derive_part_rels_path_handles_root_part() {
        let result = derive_part_rels_path("document.xml");

        assert_eq!(result, "_rels/document.xml.rels");
    }

    #[test]
    fn resolve_relative_target_handles_package_absolute_target() {
        let result = resolve_relative_target("word/document.xml", "/media/image.png");

        assert_eq!(result, "media/image.png");
    }

    #[test]
    fn resolve_relative_target_handles_root_part() {
        let result = resolve_relative_target("document.xml", "styles.xml");

        assert_eq!(result, "styles.xml");
    }

    #[test]
    fn normalize_relative_target_rejects_package_root_escape() {
        let result = normalize_relative_target("word/document.xml", "../../escape.xml");

        match result {
            Err(Error::Parse { message, position }) => {
                assert_eq!(
                    message,
                    "rels target escapes package root: word/../../escape.xml"
                );
                assert_eq!(position, None);
            }
            other => assert_eq!(format!("{other:?}"), "expected package-root escape error"),
        }
    }
}

#[cfg(test)]
#[cfg(not(coverage))]
mod tests {
    use super::*;

    #[test]
    fn derive_part_rels_path_with_directory() {
        let result = derive_part_rels_path("word/document.xml");
        assert_eq!(result, "word/_rels/document.xml.rels");
    }

    #[test]
    fn derive_part_rels_path_without_directory() {
        let result = derive_part_rels_path("foo");
        assert_eq!(result, "_rels/foo.rels");
    }

    #[test]
    fn resolve_relative_target_with_directory() {
        let result = resolve_relative_target("word/document.xml", "styles.xml");
        assert_eq!(result, "word/styles.xml");
    }

    #[test]
    fn resolve_relative_target_strips_leading_slash() {
        let result = resolve_relative_target("word/document.xml", "/word/styles.xml");
        assert_eq!(result, "word/styles.xml");
    }

    #[test]
    fn resolve_relative_target_without_directory() {
        let result = resolve_relative_target("document.xml", "styles.xml");
        assert_eq!(result, "styles.xml");
    }
}
