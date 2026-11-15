use nemo_core::together::RoomError;
use proptest::prelude::*;

use super::ServerAddress;

fn ok(text: &str) -> ServerAddress {
    ServerAddress::parse(text).expect("address should be accepted")
}

fn bad(text: &str) {
    assert_eq!(
        ServerAddress::parse(text),
        Err(RoomError::BadAddress),
        "{text:?} should be refused"
    );
}

#[test]
fn wss_accepts_host_port_and_path() {
    let a = ok("wss://Example.org:8443/rooms/ws");
    assert!(a.is_secure());
    assert_eq!(a.host(), "example.org");
    assert_eq!(a.port(), Some(8443));
    assert_eq!(a.path(), "/rooms/ws");
    assert_eq!(a.to_string(), "wss://example.org:8443/rooms/ws");
}

#[test]
fn wss_without_port_or_path_gets_root_path() {
    let a = ok("WSS://together.example.net");
    assert_eq!(a.port(), None);
    assert_eq!(a.path(), "/");
}

#[test]
fn wss_accepts_public_ip_literals() {
    ok("wss://203.0.113.7/ws");
    ok("wss://[2001:db8::1]:443/ws");
}

#[test]
fn ws_is_accepted_for_loopback_private_and_local_hosts() {
    for text in [
        "ws://localhost:8080/ws",
        "ws://127.0.0.1/ws",
        "ws://10.1.2.3:9000/ws",
        "ws://172.16.0.5/ws",
        "ws://172.31.255.254/ws",
        "ws://192.168.1.20:3000/",
        "ws://[::1]:8080/ws",
        "ws://[fd12:3456::1]/ws",
        "ws://music-box.local:8080/ws",
    ] {
        assert!(!ok(text).is_secure(), "{text}");
    }
}

#[test]
fn ws_is_refused_for_public_hosts() {
    for text in [
        "ws://example.org/ws",
        "ws://8.8.8.8/ws",
        "ws://172.32.0.1/ws",
        "ws://172.15.0.1/ws",
        "ws://169.254.1.1/ws",
        "ws://[2001:db8::1]/ws",
        "ws://local/ws",
        "ws://.local/ws",
        "ws://evil.local.example.org/ws",
        "ws://localhost.example.org/ws",
    ] {
        bad(text);
    }
}

#[test]
fn other_schemes_and_shapes_are_refused() {
    for text in [
        "",
        "example.org",
        "https://example.org/ws",
        "http://localhost/ws",
        "ftp://example.org",
        "wss://",
        "wss:///ws",
        "wss://user@example.org/ws",
        "wss://user:pw@example.org/ws",
        "wss://example.org/ws?room=1",
        "wss://example.org/ws#top",
        "wss://example.org/a b",
        "wss://example.org/\u{e9}",
        "wss://exa mple.org/ws",
        "wss://example.org:/ws",
        "wss://example.org:0/ws",
        "wss://example.org:65536/ws",
        "wss://example.org:+80/ws",
        "wss://example.org:http/ws",
        "wss://-bad.example.org/ws",
        "wss://bad-.example.org/ws",
        "wss://exa..mple.org/ws",
        "wss://example.org./ws",
        "wss://127.1/ws",
        "wss://0x7f.1/ws",
        "wss://[::1/ws",
        "wss://[not-an-ip]/ws",
        "wss://[::1]x/ws",
        "wss://example.org/a\\b",
    ] {
        bad(text);
    }
}

#[test]
fn display_round_trips() {
    for text in [
        "wss://example.org/",
        "ws://192.168.0.2:81/x/y",
        "wss://[2001:db8::1]:9/p",
    ] {
        assert_eq!(ok(text).to_string(), text);
        assert_eq!(ok(&ok(text).to_string()), ok(text));
    }
}

proptest! {
    #[test]
    fn parse_never_panics_and_accepted_text_round_trips(text in "\\PC{0,60}") {
        if let Ok(a) = ServerAddress::parse(&text) {
            prop_assert_eq!(ServerAddress::parse(&a.to_string()), Ok(a));
        }
    }

    #[test]
    fn urlish_text_never_panics(text in "(wss?|WSS?)://[a-zA-Z0-9.:\\[\\]@/?#_-]{0,40}") {
        let _ = ServerAddress::parse(&text);
    }

    #[test]
    fn ws_to_an_ipv4_is_accepted_only_when_local(a in 0u8..=255, b in 0u8..=255, c in 0u8..=255, d in 0u8..=255) {
        let ip = std::net::Ipv4Addr::new(a, b, c, d);
        let local = ip.is_loopback() || ip.is_private();
        let result = ServerAddress::parse(&format!("ws://{ip}/ws"));
        prop_assert_eq!(result.is_ok(), local);
    }

    #[test]
    fn wss_to_a_valid_name_is_accepted(
        name in "[a-z][a-z0-9]{0,10}(\\.[a-z][a-z0-9-]{0,10}[a-z0-9]){0,3}",
        port in 1u16..=65535,
        path in "(/[a-zA-Z0-9._~-]{1,8}){0,3}",
    ) {
        let text = format!("wss://{name}:{port}{path}");
        let a = ServerAddress::parse(&text).expect("valid name");
        prop_assert_eq!(a.port(), Some(port));
        prop_assert_eq!(a.host(), name.as_str());
    }
}
