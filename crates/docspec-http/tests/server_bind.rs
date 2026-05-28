//! Bind/serve tests for the `server` module.

#![allow(clippy::tests_outside_test_module, clippy::panic, clippy::expect_used)]

use tokio::net::TcpListener;

use docspec_http::server::{bind, serve, ServerConfig};

#[tokio::test]
async fn bind_succeeds_on_port_zero() {
    let listener = bind(&ServerConfig::new("127.0.0.1", 0))
        .await
        .expect("bind to port 0 should succeed");
    let addr = listener
        .local_addr()
        .expect("local_addr should succeed after bind");
    assert_ne!(addr.port(), 0, "OS should assign a non-zero port");
}

#[tokio::test]
async fn unresolvable_host_returns_error() {
    let result = serve(ServerConfig::new(
        "definitely-not-a-real-host.invalid",
        9999,
    ))
    .await;

    match result {
        Err(_) => {}
        Ok(()) => panic!("unresolvable host should return an error"),
    }
}

#[tokio::test]
async fn ipv6_loopback_binds() {
    // Reason: skip when the host environment lacks IPv6; the test exists to prove
    // that `bind()` accepts an unbracketed `::1` literal, which the old
    // `format!("{host}:{port}").parse::<SocketAddr>()` path would reject.
    if TcpListener::bind(("::1", 0)).await.is_err() {
        return;
    }

    let listener = bind(&ServerConfig::new("::1", 0))
        .await
        .expect("IPv6 bind should succeed when ::1 is available");
    let addr = listener
        .local_addr()
        .expect("local_addr should succeed after bind");
    assert!(addr.is_ipv6(), "expected an IPv6 bound address");
}
