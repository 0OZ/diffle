use super::*;

const LOOPBACK: &str = "127.0.0.1";

#[tokio::test]
async fn port_zero_lets_the_system_choose() {
    let listener = bind(LOOPBACK, PortChoice::Exact(0)).await.unwrap();
    assert_ne!(listener.local_addr().unwrap().port(), 0);
}

#[tokio::test]
async fn a_taken_exact_port_is_an_error_that_names_it() {
    let taken = bind(LOOPBACK, PortChoice::Exact(0)).await.unwrap();
    let port = taken.local_addr().unwrap().port();
    let error = bind(LOOPBACK, PortChoice::Exact(port)).await.unwrap_err();
    assert!(
        matches!(error, ServerError::PortTaken { port: taken, .. } if taken == port),
        "{error}"
    );
}

#[tokio::test]
async fn probing_skips_a_taken_default_port() {
    // The default port may be free or not on this machine; either way the result is in range.
    let first = bind(LOOPBACK, PortChoice::Probe).await.unwrap();
    let second = bind(LOOPBACK, PortChoice::Probe).await.unwrap();
    let (low, high) = PortChoice::Probe.range();
    for listener in [&first, &second] {
        let port = listener.local_addr().unwrap().port();
        assert!((low..=high).contains(&port), "{port}");
    }
    assert_ne!(first.local_addr().unwrap(), second.local_addr().unwrap());
}

#[tokio::test]
async fn an_unusable_host_is_a_bind_error_not_a_port_problem() {
    let error = bind("256.256.256.256", PortChoice::Probe)
        .await
        .unwrap_err();
    assert!(matches!(error, ServerError::Bind { .. }), "{error}");
}
