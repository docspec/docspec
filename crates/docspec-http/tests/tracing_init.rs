//! Tests for tracing subscriber initialization.

#![allow(clippy::tests_outside_test_module)]

use docspec_http::tracing_init::try_init;

#[test]
fn try_init_is_idempotent() {
    // First call may succeed or fail depending on test order — but must not panic.
    drop(try_init());
    // Second call must not panic either.
    let result = try_init();
    // If first call succeeded, second returns Err. Either way, no panic.
    drop(result);
}
