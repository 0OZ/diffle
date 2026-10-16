use std::net::SocketAddr;

use super::*;

fn url(host: &str) -> ServerUrl {
    ServerUrl::new(host, SocketAddr::from(([127, 0, 0, 1], 4966)))
}

#[test]
fn a_url_names_the_host_and_the_bound_port() {
    assert_eq!(url("127.0.0.1").to_string(), "http://127.0.0.1:4966/");
    assert_eq!(url("localhost").port(), 4966);
}

#[test]
fn an_ipv6_host_is_bracketed() {
    assert_eq!(url("::1").to_string(), "http://[::1]:4966/");
}

#[test]
fn listening_everywhere_is_reached_through_localhost() {
    assert_eq!(
        url("0.0.0.0").for_this_machine().to_string(),
        "http://localhost:4966/"
    );
    assert_eq!(
        url("::").for_this_machine().to_string(),
        "http://localhost:4966/"
    );
    assert_eq!(
        url("192.168.1.5").for_this_machine().to_string(),
        "http://192.168.1.5:4966/"
    );
}
