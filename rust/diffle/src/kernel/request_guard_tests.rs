use super::*;

fn loopback() -> RequestGuard {
    RequestGuard::new("127.0.0.1", &[], None)
}

fn proxied() -> RequestGuard {
    let public = AllowedOrigin::parse("https://review.example").unwrap();
    RequestGuard::new("127.0.0.1", &[], Some(public))
}

fn lan() -> RequestGuard {
    RequestGuard::new("0.0.0.0", &["Dev-Box".into(), "dev-box.local".into()], None)
}

fn rejected(guard: &RequestGuard, host: Option<&str>, origin: Option<&str>) -> String {
    guard.check(host, origin).unwrap_err().to_string()
}

#[test]
fn a_loopback_server_answers_loopback_hosts_only() {
    let guard = loopback();
    for host in [
        "localhost:4966",
        "127.0.0.1:4966",
        "[::1]:4966",
        "LOCALHOST",
    ] {
        assert_eq!(guard.check(Some(host), None), Ok(()), "{host}");
    }
    assert_eq!(rejected(&guard, None, None), "missing Host header");
    assert_eq!(
        rejected(&guard, Some("evil.example"), None),
        "forbidden Host \"evil.example\"; if a reverse proxy sent it, trust that origin with --allowed-origin"
    );
    // A rebinding page can name a LAN address that resolves back to this machine.
    assert!(guard.check(Some("192.168.1.5:4966"), None).is_err());
}

#[test]
fn a_page_may_call_only_from_its_own_host() {
    let guard = loopback();
    assert_eq!(
        guard.check(Some("localhost:4966"), Some("http://localhost:4966")),
        Ok(())
    );
    assert_eq!(
        rejected(&guard, Some("localhost:4966"), Some("http://evil.example")),
        "forbidden Origin \"http://evil.example\" for Host \"localhost:4966\"; if a reverse proxy sent it, trust that origin with --allowed-origin"
    );
    assert!(guard.check(Some("localhost:4966"), Some("null")).is_err());
    assert!(
        guard
            .check(Some("localhost:4966"), Some("http://localhost:3000"))
            .is_err()
    );
}

#[test]
fn a_bound_network_server_answers_ip_literals_and_its_own_names() {
    let guard = lan();
    for host in [
        "10.0.0.7:4966",
        "[fe80::1]:4966",
        "dev-box:4966",
        "dev-box.local",
        "0.0.0.0:1",
    ] {
        assert_eq!(guard.check(Some(host), None), Ok(()), "{host}");
    }
    assert!(guard.check(Some("evil.example"), None).is_err());
}

#[test]
fn a_configured_public_origin_is_trusted_with_its_host() {
    let guard = proxied();
    assert_eq!(
        guard.check(Some("review.example"), Some("https://review.example")),
        Ok(())
    );
    // The proxy may also rewrite Host to the upstream.
    assert_eq!(
        guard.check(Some("localhost:4966"), Some("https://review.example")),
        Ok(())
    );
    assert_eq!(guard.check(Some("review.example"), None), Ok(()));
}

#[test]
fn a_public_host_does_not_trust_another_scheme_on_that_origin() {
    let guard = proxied();
    assert_eq!(
        rejected(
            &guard,
            Some("review.example"),
            Some("http://review.example")
        ),
        "forbidden Origin \"http://review.example\" for Host \"review.example\"; allowed origin is \"https://review.example\""
    );
    assert!(guard.check(Some("other.example"), None).is_err());
}

#[test]
fn the_machine_names_include_the_short_and_mdns_forms() {
    let names = own_names();
    assert_eq!(names.len(), 3);
    assert!(names[2].ends_with(".local"));
    assert!(!names[1].contains('.'));
}

#[test]
fn hostnames_lose_ports_and_brackets() {
    assert_eq!(hostname_of("localhost:80"), "localhost");
    assert_eq!(hostname_of("[::1]:80"), "::1");
    assert_eq!(hostname_of("::1"), "::1");
    assert_eq!(hostname_of("example"), "example");
}
