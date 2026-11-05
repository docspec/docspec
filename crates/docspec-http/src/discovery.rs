//! Machine-readable description of what this server converts.
//!
//! The media type lists come from the tables in [`crate::mime`], so discovery
//! responses cannot disagree with what `POST /conversion` accepts. Each body
//! is a struct whose `ToSchema` derive is what the `OpenAPI` document publishes,
//! so a field added here shows up in the API description too.

use docspec_json::{JsonBackend, JsonEmitter, StrusonBackend};
use utoipa::ToSchema;

use crate::mime;

/// Version of this server, as reported by discovery responses.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Media types `POST /conversion` reads and writes.
#[derive(ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MediaTypes {
    /// Media types accepted on `Content-Type`.
    #[schema(min_items = 1)]
    inputs: Vec<&'static str>,
    /// Media types accepted on `Accept`, aliases included.
    #[schema(min_items = 1)]
    outputs: Vec<&'static str>,
}

impl MediaTypes {
    /// The media types this build supports.
    fn current() -> Self {
        Self {
            inputs: mime::input_mimes().collect(),
            outputs: mime::output_mimes().collect(),
        }
    }
}

/// Writes the `"inputs"` and `"outputs"` arrays into the open object.
fn write_media_types<B>(
    object: &mut JsonEmitter<B>,
    inputs: &[&str],
    outputs: &[&str],
) -> docspec_core::Result<()>
where
    B: JsonBackend,
{
    object
        .key("inputs")
        .array(|array| inputs.iter().try_for_each(|input| array.value(*input)))?;
    object
        .key("outputs")
        .array(|array| outputs.iter().try_for_each(|output| array.value(*output)))
}

/// `GET /conversion` body: the server version and the media types it converts.
#[derive(ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConversionFormats {
    /// Version of the running server.
    version: &'static str,
    /// Media types accepted on `Content-Type`.
    #[schema(min_items = 1)]
    inputs: Vec<&'static str>,
    /// Media types accepted on `Accept`, aliases included.
    #[schema(min_items = 1)]
    outputs: Vec<&'static str>,
}

impl ConversionFormats {
    /// The formats of this build.
    pub(crate) fn current() -> Self {
        let MediaTypes { inputs, outputs } = MediaTypes::current();
        Self {
            version: VERSION,
            inputs,
            outputs,
        }
    }

    /// Serializes this body as JSON.
    pub(crate) fn to_json(&self) -> docspec_core::Result<Vec<u8>> {
        let mut emitter = JsonEmitter::new(StrusonBackend::new(Vec::new()));
        emitter.object(|object| {
            object.key("version").value(self.version)?;
            write_media_types(object, &self.inputs, &self.outputs)
        })?;
        emitter.finish()
    }
}

/// Links from the discovery document to the server's other resources.
///
/// Each is relative to the discovery document's own URL (RFC 3986 §5), so
/// `../` reaches the server root and links still resolve behind a path prefix.
#[derive(ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Links {
    /// The conversion endpoint.
    #[schema(format = "uri-reference")]
    conversion: &'static str,
    /// The liveness check.
    #[schema(format = "uri-reference")]
    health: &'static str,
}

/// `GET /.well-known/docspec` body: the version, links to the server's
/// resources, and the media types `/conversion` supports.
#[derive(ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WellKnown {
    /// Version of the running server.
    version: &'static str,
    /// Links to the server's resources.
    links: Links,
    /// Media types `POST /conversion` reads and writes.
    conversion: MediaTypes,
}

impl WellKnown {
    /// The discovery document of this build.
    pub(crate) fn current() -> Self {
        Self {
            version: VERSION,
            links: Links {
                conversion: "../conversion",
                health: "../health",
            },
            conversion: MediaTypes::current(),
        }
    }

    /// Serializes this body as JSON.
    pub(crate) fn to_json(&self) -> docspec_core::Result<Vec<u8>> {
        let mut emitter = JsonEmitter::new(StrusonBackend::new(Vec::new()));
        emitter.object(|object| {
            object.key("version").value(self.version)?;
            object.key("links").object(|links| {
                links.key("conversion").value(self.links.conversion)?;
                links.key("health").value(self.links.health)
            })?;
            object.key("conversion").object(|conversion| {
                write_media_types(
                    conversion,
                    &self.conversion.inputs,
                    &self.conversion.outputs,
                )
            })
        })?;
        emitter.finish()
    }
}
