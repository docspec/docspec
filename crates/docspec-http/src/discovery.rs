//! Machine-readable description of what this server converts.
//!
//! The media type lists come from the tables in [`crate::mime`], so discovery
//! responses cannot disagree with what `POST /conversion` accepts. Each body
//! is a struct with a `ToSchema` derive: the `OpenAPI` document publishes the
//! one of `GET /conversion`, so a field added here shows up there too. The
//! `/.well-known/*` schemas are not published; tests check the bodies
//! against them.

use serde::Serialize;
use utoipa::ToSchema;

use crate::mime;

/// Version of this server, as reported by discovery responses.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Media types `POST /conversion` reads and writes.
#[derive(Serialize, ToSchema)]
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

/// `GET /conversion` body: the server version and the media types it converts.
#[derive(Serialize, ToSchema)]
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
    pub(crate) fn to_json(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }
}

/// Links from the discovery document to the server's other resources.
///
/// Each is relative to the discovery document's own URL (RFC 3986 §5), so
/// `../` reaches the server root and links still resolve behind a path prefix.
#[derive(Serialize, ToSchema)]
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
#[derive(Serialize, ToSchema)]
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
    pub(crate) fn to_json(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }
}

/// A link target: where it points and the media type found there.
#[derive(Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TargetLink {
    /// Target URI, relative to the catalog's URL.
    #[schema(format = "uri-reference")]
    href: &'static str,
    /// Media type of the target.
    #[serde(rename = "type")]
    media_type: &'static str,
}

/// The links of one API: its endpoint and the description of it.
#[derive(Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LinkContext {
    /// The API endpoint, relative to the catalog's URL.
    #[schema(format = "uri-reference")]
    anchor: &'static str,
    /// Machine-readable descriptions of the API.
    #[serde(rename = "service-desc")]
    service_desc: Vec<TargetLink>,
}

/// `GET /.well-known/api-catalog` body: an RFC 9727 API catalog, as an
/// RFC 9264 JSON linkset.
///
/// Links are relative to the catalog's URL, so `../` reaches the server root
/// both at the origin root and under a path prefix. This deliberately deviates
/// from RFC 9264 §4.2.2 and §4.2.3, which say `anchor` and `href` SHOULD NOT be
/// relative: the server has no configured public base URL, and building one
/// from the `Host` header would trust client input.
#[derive(Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApiCatalog {
    /// One link context per API.
    linkset: Vec<LinkContext>,
}

impl ApiCatalog {
    /// The catalog of this build: the conversion API and its `OpenAPI` description.
    pub(crate) fn current() -> Self {
        Self {
            linkset: vec![LinkContext {
                anchor: "../conversion",
                service_desc: vec![TargetLink {
                    href: "../openapi.json",
                    media_type: crate::openapi::MIME_OPENAPI,
                }],
            }],
        }
    }

    /// Serializes this body as JSON.
    pub(crate) fn to_json(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::tests_outside_test_module)]

    use serde_json::{json, Value};
    use utoipa::openapi::ComponentsBuilder;

    use super::*;

    /// Validation errors of `body` against `T`'s own schema.
    fn schema_errors<T>(body: &[u8]) -> Vec<String>
    where
        T: ToSchema,
    {
        let mut nested = Vec::new();
        T::schemas(&mut nested);
        let components = ComponentsBuilder::new()
            .schema(T::name(), T::schema())
            .schemas_from_iter(nested)
            .build();
        let schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": format!("#/components/schemas/{}", T::name()),
            "components": components,
        });
        let body: Value = serde_json::from_slice(body).expect("valid JSON");
        let validator = jsonschema::validator_for(&schema).expect("valid JSON Schema");
        validator
            .iter_errors(&body)
            .map(|error| format!("{error} at {}", error.instance_path()))
            .collect()
    }

    #[test]
    fn well_known_body_matches_its_schema() {
        let body = WellKnown::current().to_json().expect("serializes");

        assert_eq!(schema_errors::<WellKnown>(&body), Vec::<String>::new());
    }

    #[test]
    fn api_catalog_body_matches_its_schema() {
        let body = ApiCatalog::current().to_json().expect("serializes");

        assert_eq!(schema_errors::<ApiCatalog>(&body), Vec::<String>::new());
    }
}
