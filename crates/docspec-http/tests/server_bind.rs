//! Bind/serve tests for the `server` module.

#![allow(clippy::tests_outside_test_module, clippy::panic, clippy::expect_used)]

use tokio::net::TcpListener;

use docspec_http::{
    metrics::install_global,
    server::{bind, serve, ServerConfig, ServerError},
};

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
async fn unresolvable_host_returns_listen_error_before_global_metrics_install() {
    let result = serve(ServerConfig::new(
        "definitely-not-a-real-host.invalid",
        9999,
    ))
    .await;

    match result {
        Err(ServerError::Listen(_)) => {}
        Err(error) => panic!("expected listen error, got {error:?}"),
        Ok(()) => panic!("unresolvable host should return an error"),
    }

    install_global().expect("bind failure should not install global metrics recorder");

    let metrics_result = serve(ServerConfig::new("127.0.0.1", 0)).await;
    match metrics_result {
        Err(error @ ServerError::MetricsInit(_)) => assert!(
            core::error::Error::source(&error).is_some(),
            "metrics init error should preserve the BuildError source"
        ),
        Err(error) => panic!("expected metrics init error, got {error:?}"),
        Ok(()) => panic!("global recorder conflict should return an error"),
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
