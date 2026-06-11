//! In-memory DOCX fixture helpers for tests.
//!
//! Thin re-export of the workspace-internal `docspec-test-utils` crate.

// Reason: this module is included by every integration test binary in the crate.
// Each binary uses some subset of the re-exports, so the unused-import warning
// is unavoidable per-binary without splitting the module per test file.
#[allow(unused_imports)]
pub use docspec_test_utils::{synth_docx, synth_docx_with_entries};
