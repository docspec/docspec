#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! HTTP API server for `DocSpec` document conversion.

pub mod cache;
pub mod error;
pub mod format;
pub mod handlers;
pub mod router;
pub mod server;
pub mod tracing_init;

pub use server::serve;
