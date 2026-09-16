use wasm_bindgen::JsValue;

use crate::error::{js_error, ErrorCode};

/// The compiled input formats.
///
/// Uninhabited when no reader is compiled in, which is what lets
/// `convert_stream` discharge that case with `match input {}`.
pub(crate) enum Input {
    #[cfg(feature = "docx-reader")]
    Docx,
    #[cfg(feature = "html-reader")]
    Html,
    #[cfg(feature = "markdown-reader")]
    Markdown,
}

/// The compiled output formats.
///
/// This mirrors [`Input`] rather than using `docspec::OutputFormat` directly so
/// that it is uninhabited when no writer is compiled in. `docspec::OutputFormat`
/// is `#[non_exhaustive]`, so a match on it from this crate would still demand a
/// wildcard arm and could not prove the no-writer case away.
pub(crate) enum Output {
    #[cfg(feature = "blocknote-writer")]
    Blocknote,
    #[cfg(feature = "html-writer")]
    Html,
    #[cfg(feature = "markdown-writer")]
    Markdown,
    #[cfg(feature = "oxa-writer")]
    Oxa,
    #[cfg(feature = "pandoc-native-writer")]
    PandocNative,
}

#[cfg(writer)]
impl Output {
    pub(crate) const fn into_format(self) -> docspec::OutputFormat {
        match self {
            #[cfg(feature = "blocknote-writer")]
            Self::Blocknote => docspec::OutputFormat::Blocknote,
            #[cfg(feature = "html-writer")]
            Self::Html => docspec::OutputFormat::Html,
            #[cfg(feature = "markdown-writer")]
            Self::Markdown => docspec::OutputFormat::Markdown,
            #[cfg(feature = "oxa-writer")]
            Self::Oxa => docspec::OutputFormat::Oxa,
            #[cfg(feature = "pandoc-native-writer")]
            Self::PandocNative => docspec::OutputFormat::PandocNative,
        }
    }
}

pub(crate) fn input_formats() -> &'static [&'static str] {
    &[
        #[cfg(feature = "docx-reader")]
        "docx",
        #[cfg(feature = "html-reader")]
        "html",
        #[cfg(feature = "markdown-reader")]
        "markdown",
    ]
}

pub(crate) fn output_formats() -> &'static [&'static str] {
    &[
        #[cfg(feature = "blocknote-writer")]
        "blocknote",
        #[cfg(feature = "html-writer")]
        "html",
        #[cfg(feature = "markdown-writer")]
        "markdown",
        #[cfg(feature = "oxa-writer")]
        "oxa",
        #[cfg(feature = "pandoc-native-writer")]
        "pandoc-native",
    ]
}

pub(crate) fn input_format(name: &str) -> Result<Input, JsValue> {
    match name {
        #[cfg(feature = "docx-reader")]
        "docx" => Ok(Input::Docx),
        #[cfg(feature = "html-reader")]
        "html" => Ok(Input::Html),
        #[cfg(feature = "markdown-reader")]
        "markdown" => Ok(Input::Markdown),
        _ => Err(js_error(
            ErrorCode::UnsupportedInputFormat,
            &format!("unsupported input format: {name}"),
        )),
    }
}

pub(crate) fn output_format(name: &str) -> Result<Output, JsValue> {
    match name {
        #[cfg(feature = "blocknote-writer")]
        "blocknote" => Ok(Output::Blocknote),
        #[cfg(feature = "html-writer")]
        "html" => Ok(Output::Html),
        #[cfg(feature = "markdown-writer")]
        "markdown" => Ok(Output::Markdown),
        #[cfg(feature = "oxa-writer")]
        "oxa" => Ok(Output::Oxa),
        #[cfg(feature = "pandoc-native-writer")]
        "pandoc-native" => Ok(Output::PandocNative),
        _ => Err(js_error(
            ErrorCode::UnsupportedOutputFormat,
            &format!("unsupported output format: {name}"),
        )),
    }
}
