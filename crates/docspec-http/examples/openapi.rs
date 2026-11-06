//! Writes the `OpenAPI` document that `GET /openapi.json` serves to stdout.
//!
//! Usage: `cargo run -p docspec-http --example openapi > openapi.json`.

use std::io::Write as _;

fn main() -> Result<(), Box<dyn core::error::Error>> {
    let document = docspec_http::openapi_json()?;
    std::io::stdout().write_all(document)?;
    Ok(())
}
