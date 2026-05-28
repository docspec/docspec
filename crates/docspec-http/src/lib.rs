#![forbid(unsafe_code)]
#![deny(clippy::min_ident_chars)]
//! HTTP API server for `DocSpec` document conversion.

pub mod cache;
pub mod error;
pub mod format;
pub mod handlers;
pub mod mime_parser;
pub mod router;
pub mod server;
pub mod tracing_init;

pub use server::serve;
