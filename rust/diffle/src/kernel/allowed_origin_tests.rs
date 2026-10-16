use super::*;

fn parsed(text: &str) -> AllowedOrigin {
    AllowedOrigin::parse(text).unwrap()
}

#[test]
fn an_origin_is_normalized_like_a_browser_writes_it() {
    assert_eq!(
        parsed("https://Proxy.Example").as_str(),
        "https://proxy.example"
    );
    assert_eq!(
        parsed("https://proxy.example/").as_str(),
        "https://proxy.example"
    );
    assert_eq!(
        parsed("https://proxy.example:443").as_str(),
        "https://proxy.example"
    );
    assert_eq!(parsed("http://proxy.example:80").host(), "proxy.example");
    assert_eq!(
        parsed("http://proxy.example:8080").host(),
        "proxy.example:8080"
    );
    assert_eq!(parsed("HTTP://[::1]:3000").as_str(), "http://[::1]:3000");
}

#[test]
fn anything_beyond_scheme_host_and_port_is_refused() {
    for text in [
        "proxy.example",
        "ftp://proxy.example",
        "https://",
        "https://user@proxy.example",
        "https://proxy.example/path",
        "https://proxy.example?x=1",
        "https://proxy.example#top",
        "https://proxy.example:port",
        "https://proxy.example:99999",
        "null",
    ] {
        assert_eq!(AllowedOrigin::parse(text), Err(InvalidOrigin), "{text}");
    }
}

#[test]
fn a_request_origin_names_its_host() {
    assert_eq!(
        origin_host("http://localhost:4966").as_deref(),
        Some("localhost:4966")
    );
    assert_eq!(
        origin_host("https://App.Example:443").as_deref(),
        Some("app.example")
    );
    assert_eq!(origin_host("null"), None);
    assert_eq!(origin_host("file:///x"), None);
}
