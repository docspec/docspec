use wasm_bindgen::JsValue;

use crate::error::{js_error, ErrorCode};

pub(crate) enum Input {
    #[cfg(feature = "docx-reader")]
    Docx,
    #[cfg(feature = "html-reader")]
    Html,
    #[cfg(feature = "markdown-reader")]
    Markdown,
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

pub(crate) fn output_format(name: &str) -> Result<docspec::OutputFormat, JsValue> {
    match name {
        #[cfg(feature = "blocknote-writer")]
        "blocknote" => Ok(docspec::OutputFormat::Blocknote),
        #[cfg(feature = "html-writer")]
        "html" => Ok(docspec::OutputFormat::Html),
        #[cfg(feature = "markdown-writer")]
        "markdown" => Ok(docspec::OutputFormat::Markdown),
        #[cfg(feature = "oxa-writer")]
        "oxa" => Ok(docspec::OutputFormat::Oxa),
        #[cfg(feature = "pandoc-native-writer")]
        "pandoc-native" => Ok(docspec::OutputFormat::PandocNative),
        _ => Err(js_error(
            ErrorCode::UnsupportedOutputFormat,
            &format!("unsupported output format: {name}"),
        )),
    }
}
