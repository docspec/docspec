//! The `OpenAPI` 3.1 description of this server, served at `GET /openapi.json`.
//!
//! Paths, operations and response schemas come from the `#[utoipa::path]`
//! annotations of the handlers registered in [`crate::router::routes`], so a
//! route cannot be served without being described. The media types of
//! `POST /conversion` are filled in here from the tables in [`crate::mime`],
//! which is also what validates requests, so the two cannot disagree.

use std::sync::LazyLock;

use utoipa::openapi::{
    path::Operation,
    request_body::RequestBodyBuilder,
    schema::{ObjectBuilder, Type},
    ContentBuilder, InfoBuilder, OpenApi, OpenApiBuilder, RefOr, Required, Schema,
};

use crate::{discovery::VERSION, error::HttpError, mime};

/// Media type of the `OpenAPI` document.
pub(crate) const MIME_OPENAPI: &str = "application/vnd.oai.openapi+json;version=3.1";

/// Error building the document: an annotated operation the code expects is missing.
#[derive(Debug, thiserror::Error)]
#[error("OpenAPI document has no {0} operation")]
pub(crate) struct MissingOperation(&'static str);

/// The serialized document, built on first use.
static DOCUMENT: LazyLock<Result<Vec<u8>, String>> =
    LazyLock::new(|| build().map_err(|error| error.to_string()));

/// Document root: everything not derived from the route table.
pub(crate) fn base() -> OpenApi {
    OpenApiBuilder::new()
        .info(
            InfoBuilder::new()
                .title("DocSpec HTTP API")
                .version(VERSION)
                .description(Some(
                    "Streaming document conversion. Errors are RFC 9457 problem details. \
                     `HEAD` is served wherever `GET` is. Undocumented paths answer 404 and \
                     other undocumented methods 405, both as problem details. `GET /metrics` is served only when metrics are enabled \
                     and is not described here.",
                )),
        )
        .build()
}

/// Returns the serialized document.
///
/// # Errors
///
/// Returns [`HttpError::Internal`] if the document could not be built.
pub(crate) fn json() -> Result<&'static [u8], HttpError> {
    DOCUMENT
        .as_deref()
        .map_err(|message| HttpError::internal(std::io::Error::other(message.clone())))
}

/// Builds the document from the route table and the MIME tables.
fn build() -> Result<Vec<u8>, Box<dyn core::error::Error + Send + Sync>> {
    let (_, mut api) = crate::router::routes().split_for_parts();
    let post = api
        .paths
        .paths
        .get_mut("/conversion")
        .and_then(|item| item.post.as_mut())
        .ok_or(MissingOperation("POST /conversion"))?;
    describe_conversion_media(post)?;
    Ok(serde_json::to_vec(&api)?)
}

/// Fills in the request and success media types of `POST /conversion` from
/// the MIME tables.
///
/// `Accept` is not declared as a parameter: `OpenAPI` 3.1 says header
/// parameters named `Accept` are ignored, so the operation description says
/// how it selects the writer instead.
fn describe_conversion_media(post: &mut Operation) -> Result<(), MissingOperation> {
    let request = mime::input_mimes().fold(
        RequestBodyBuilder::new()
            .description(Some(
                "The document to convert. `Content-Type` selects the reader.",
            ))
            .required(Some(Required::True)),
        |request, input| {
            request.content(
                input,
                ContentBuilder::new().schema(body_schema(input)).build(),
            )
        },
    );
    post.request_body = Some(RefOr::T(request.build()));

    let Some(RefOr::T(ok)) = post.responses.responses.get_mut("200") else {
        return Err(MissingOperation("POST /conversion 200 response"));
    };
    ok.content.extend(mime::served_output_mimes().map(|output| {
        let content = ContentBuilder::new().schema(body_schema(output)).build();
        (output.to_owned(), RefOr::T(content))
    }));
    Ok(())
}

/// Schema of a request or response body of the given media type: a string for
/// text types, and unconstrained for binary and JSON documents.
fn body_schema(media_type: &str) -> Option<RefOr<Schema>> {
    media_type
        .starts_with("text/")
        .then(|| ObjectBuilder::new().schema_type(Type::String).into())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::tests_outside_test_module,
        clippy::unwrap_used,
        clippy::expect_used
    )]

    use axum::response::IntoResponse as _;
    use serde_json::Value;

    use super::*;
    use crate::error::{HttpError, InputProblem};

    fn document() -> Value {
        serde_json::from_slice(json().expect("document builds")).expect("valid JSON")
    }

    fn post_conversion() -> Value {
        document()
            .pointer("/paths/~1conversion/post")
            .cloned()
            .expect("POST /conversion documented")
    }

    fn keys(value: Option<&Value>) -> Vec<String> {
        value
            .and_then(Value::as_object)
            .expect("object")
            .keys()
            .cloned()
            .collect()
    }

    fn sorted(items: impl Iterator<Item = &'static str>) -> Vec<String> {
        let mut items: Vec<String> = items.map(str::to_owned).collect();
        items.sort();
        items
    }

    #[test]
    fn request_media_types_are_the_input_table() {
        assert_eq!(
            keys(post_conversion().pointer("/requestBody/content")),
            sorted(mime::input_mimes())
        );
    }

    #[test]
    fn success_media_types_are_the_served_output_table() {
        assert_eq!(
            keys(post_conversion().pointer("/responses/200/content")),
            sorted(mime::served_output_mimes())
        );
    }

    #[test]
    fn accept_is_not_declared_as_a_parameter() {
        assert_eq!(post_conversion().get("parameters"), None);
    }

    #[test]
    fn error_statuses_are_the_ones_conversion_produces() {
        let produced: Vec<String> = [
            HttpError::EmptyBody,
            HttpError::NotAcceptable,
            HttpError::UnsupportedMediaType { received: None },
            HttpError::Unprocessable {
                kind: InputProblem::Malformed,
                detail: String::new(),
            },
            HttpError::internal(std::io::Error::other("test")),
        ]
        .into_iter()
        .map(|error| error.into_response().status().as_str().to_owned())
        .collect();
        let documented: Vec<String> = keys(post_conversion().pointer("/responses"))
            .into_iter()
            .filter(|status| status != "200")
            .collect();

        assert_eq!(documented, produced);
    }

    #[test]
    fn text_bodies_are_strings_and_others_unconstrained() {
        assert_eq!(
            serde_json::to_value(body_schema("text/html")).unwrap(),
            serde_json::json!({"type": "string"})
        );
        assert!(body_schema("application/vnd.oxa+json").is_none());
    }
}
