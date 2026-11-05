//! Machine-readable description of what this server converts.
//!
//! The media type lists come from the tables in [`crate::mime`], so discovery
//! responses cannot disagree with what `POST /conversion` accepts.

use docspec_json::{JsonBackend, JsonEmitter, StrusonBackend};

use crate::mime;

/// Version of this server, as reported by discovery responses.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Writes the `"inputs"` and `"outputs"` media type arrays into the open object.
pub(crate) fn write_media_types<B>(object: &mut JsonEmitter<B>) -> docspec_core::Result<()>
where
    B: JsonBackend,
{
    object
        .key("inputs")
        .array(|array| mime::input_mimes().try_for_each(|input| array.value(input)))?;
    object
        .key("outputs")
        .array(|array| mime::output_mimes().try_for_each(|output| array.value(output)))
}

/// Serializes the `GET /conversion` body: the version and supported media types.
pub(crate) fn conversion_formats_json() -> docspec_core::Result<Vec<u8>> {
    let mut emitter = JsonEmitter::new(StrusonBackend::new(Vec::new()));
    emitter.object(|object| {
        object.key("version").value(VERSION)?;
        write_media_types(object)
    })?;
    emitter.finish()
}
