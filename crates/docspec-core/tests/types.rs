//! Tests.

#[cfg(test)]
mod tests {
    use docspec_core::*;

    #[test]
    fn author_clone() {
        let author = Author {
            name: "Alice".to_string(),
            email: Some("alice@example.com".to_string()),
        };
        let cloned = author.clone();
        assert_eq!(author, cloned);
    }

    #[test]
    fn author_constructor() {
        let author = Author {
            name: "John Doe".to_string(),
            email: Some("john@example.com".to_string()),
        };
        assert_eq!(author.name, "John Doe");
        assert_eq!(author.email, Some("john@example.com".to_string()));
    }

    #[test]
    fn author_equality() {
        let author1 = Author {
            name: "Bob".to_string(),
            email: Some("bob@example.com".to_string()),
        };
        let author2 = Author {
            name: "Bob".to_string(),
            email: Some("bob@example.com".to_string()),
        };
        let author3 = Author {
            name: "Bob".to_string(),
            email: None,
        };
        assert_eq!(author1, author2);
        assert_ne!(author1, author3);
    }

    #[test]
    fn author_without_email() {
        let author = Author {
            name: "Jane Smith".to_string(),
            email: None,
        };
        assert_eq!(author.name, "Jane Smith");
        assert_eq!(author.email, None);
    }

    #[test]
    fn color_clone() {
        let color = Color::Rgb {
            r: 255,
            g: 128,
            b: 0,
        };
        let cloned = color.clone();
        assert_eq!(color, cloned);
    }

    #[test]
    fn color_equality() {
        let color1 = Color::Rgb { r: 255, g: 0, b: 0 };
        let color2 = Color::Rgb { r: 255, g: 0, b: 0 };
        let color3 = Color::Rgb { r: 0, g: 255, b: 0 };
        assert_eq!(color1, color2);
        assert_ne!(color1, color3);
    }

    #[test]
    fn document_meta_clone() {
        let meta = DocumentMeta {
            title: Some("Test".to_string()),
            authors: Some(vec![Author {
                name: "Test Author".to_string(),
                email: None,
            }]),
            description: Some("Test Description".to_string()),
        };
        let cloned = meta.clone();
        assert_eq!(meta, cloned);
    }

    #[test]
    fn document_meta_constructor() {
        let meta = DocumentMeta {
            title: Some("My Document".to_string()),
            authors: Some(vec![Author {
                name: "Author One".to_string(),
                email: Some("author@example.com".to_string()),
            }]),
            description: Some("A test document".to_string()),
        };
        assert_eq!(meta.title, Some("My Document".to_string()));
        assert!(meta.authors.is_some());
        assert_eq!(meta.description, Some("A test document".to_string()));
    }

    #[test]
    fn document_meta_empty() {
        let meta = DocumentMeta {
            title: None,
            authors: None,
            description: None,
        };
        assert_eq!(meta.title, None);
        assert_eq!(meta.authors, None);
        assert_eq!(meta.description, None);
    }

    #[test]
    fn document_meta_equality() {
        let meta1 = DocumentMeta {
            title: Some("Title".to_string()),
            authors: None,
            description: None,
        };
        let meta2 = DocumentMeta {
            title: Some("Title".to_string()),
            authors: None,
            description: None,
        };
        let meta3 = DocumentMeta {
            title: Some("Different".to_string()),
            authors: None,
            description: None,
        };
        assert_eq!(meta1, meta2);
        assert_ne!(meta1, meta3);
    }

    #[test]
    fn image_source_asset_clone() {
        let source = ImageSource::Asset {
            asset_id: "img_001".to_string(),
        };
        let cloned = source.clone();
        assert_eq!(source, cloned);
    }

    #[test]
    fn image_source_uri_clone() {
        let source = ImageSource::Uri {
            uri: "https://example.com/image.png".to_string(),
        };
        let cloned = source.clone();
        assert_eq!(source, cloned);
    }

    #[test]
    fn image_source_variants() {
        let asset = ImageSource::Asset {
            asset_id: "id1".to_string(),
        };
        let uri = ImageSource::Uri {
            uri: "http://example.com".to_string(),
        };
        assert_ne!(asset, uri);
    }

    #[test]
    fn ordered_list_style_clone() {
        let style = OrderedListStyle::Decimal;
        let cloned = style.clone();
        assert_eq!(style, cloned);
    }

    #[test]
    fn ordered_list_style_variants() {
        assert_eq!(OrderedListStyle::Decimal, OrderedListStyle::Decimal);
        assert_ne!(OrderedListStyle::Decimal, OrderedListStyle::LowerAlpha);
        assert_eq!(OrderedListStyle::LowerRoman, OrderedListStyle::LowerRoman);
        assert_ne!(OrderedListStyle::UpperAlpha, OrderedListStyle::UpperRoman);
    }

    #[test]
    fn unordered_list_style_clone() {
        let style = UnorderedListStyle::Disc;
        let cloned = style.clone();
        assert_eq!(style, cloned);
    }

    #[test]
    fn unordered_list_style_variants() {
        assert_eq!(UnorderedListStyle::Disc, UnorderedListStyle::Disc);
        assert_ne!(UnorderedListStyle::Disc, UnorderedListStyle::Circle);
        assert_ne!(UnorderedListStyle::Circle, UnorderedListStyle::Square);
    }

    #[test]
    fn table_header_scope_clone() {
        let scope = TableHeaderScope::Column;
        let cloned = scope.clone();
        assert_eq!(scope, cloned);
    }

    #[test]
    fn table_header_scope_variants() {
        assert_eq!(TableHeaderScope::Column, TableHeaderScope::Column);
        assert_ne!(TableHeaderScope::Column, TableHeaderScope::Row);
    }

    #[test]
    fn text_alignment_clone() {
        let alignment = TextAlignment::Center;
        let cloned = alignment.clone();
        assert_eq!(alignment, cloned);
    }

    #[test]
    fn text_alignment_variants() {
        assert_eq!(TextAlignment::Left, TextAlignment::Left);
        assert_ne!(TextAlignment::Left, TextAlignment::Right);
        assert_eq!(TextAlignment::Justify, TextAlignment::Justify);
    }
}
