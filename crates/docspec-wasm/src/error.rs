use wasm_bindgen::{JsCast as _, JsValue};

#[derive(Clone, Copy)]
pub(crate) enum ErrorCode {
    #[cfg(conversion)]
    Conversion,
    InvalidArgument,
    #[cfg(conversion)]
    Io,
    UnsupportedInputFormat,
    UnsupportedOutputFormat,
}

impl ErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            #[cfg(conversion)]
            Self::Conversion => "CONVERSION_ERROR",
            Self::InvalidArgument => "INVALID_ARGUMENT",
            #[cfg(conversion)]
            Self::Io => "IO_ERROR",
            Self::UnsupportedInputFormat => "UNSUPPORTED_INPUT_FORMAT",
            Self::UnsupportedOutputFormat => "UNSUPPORTED_OUTPUT_FORMAT",
        }
    }
}

pub(crate) fn js_error(code: ErrorCode, message: &str) -> JsValue {
    let error = js_sys::Error::new(message);
    match js_sys::Reflect::set(
        error.as_ref(),
        &JsValue::from_str("code"),
        &JsValue::from_str(code.as_str()),
    ) {
        Ok(_set) => {}
        Err(_set) => {}
    }
    error.unchecked_into()
}

#[cfg(conversion)]
pub(crate) fn exception_message(value: &JsValue) -> String {
    value
        .dyn_ref::<js_sys::Error>()
        .map(js_sys::Error::message)
        .map(String::from)
        .or_else(|| value.as_string())
        .unwrap_or_else(|| String::from("host callback threw an exception"))
}
