//! HTTP API server for `DocSpec` document conversion.
//!
//! Exposes the `DocSpec` streaming pipeline over HTTP using Axum 0.8.
//! Accepts `text/markdown` input and returns `application/vnd.docspec.blocknote+json`.

// Reason: This is a server crate targeting std environments, not embedded/no_std.
#![allow(clippy::std_instead_of_core)]

pub mod error;
pub mod format;
pub mod handler;
pub mod router;
pub mod server;
pub mod tracing_init;

pub use server::serve;
