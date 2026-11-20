use proptest::prelude::*;

use super::Redactor;

const HOME: &str = "/home/alice";

fn r() -> Redactor {
    Redactor::with_home(HOME)
}

#[track_caller]
fn assert_gone(input: &str, secrets: &[&str]) {
    let out = r().redact(input);
    for secret in secrets {
        assert!(
            !out.contains(secret),
            "`{secret}` survived in `{out}` (from `{input}`)"
        );
    }
    assert_eq!(r().redact(&out), out, "not idempotent for `{input}`");
}

#[test]
fn urls_keep_only_the_host() {
    let r = r();
    assert_eq!(
        r.redact("GET https://music.youtube.com/youtubei/v1/search?key=AIzaSyA-secret&prettyPrint=false 200"),
        "GET music.youtube.com 200"
    );
    assert_eq!(
        r.redact("error sending request for url (https://user:pw@api-v2.soundcloud.com:443/search?q=x#frag): timed out"),
        "error sending request for url (api-v2.soundcloud.com): timed out"
    );
    assert_eq!(r.redact("see http://127.0.0.1:8080/x."), "see 127.0.0.1");
    assert_eq!(r.redact("host https://example.com."), "host example.com.");
    assert_eq!(
        r.redact("file:///home/alice/Music/a.mp3 failed"),
        "[path] failed"
    );
    assert_eq!(r.redact("odd https://exa mple.com/x"), "odd exa mple.com/x");
    assert_eq!(r.redact("v6 http://[::1]:9/x ok"), "v6 [::1] ok");
}

#[test]
fn queries_without_a_url_are_cut() {
    assert_eq!(
        r().redact("/youtubei/v1/player?key=abc&x=1 done"),
        "/youtubei/v1/player?[query] done"
    );
    assert_eq!(r().redact("is it ok?"), "is it ok?");
}

#[test]
fn bearer_and_cookie_text_is_removed() {
    assert_gone(
        "Authorization: Bearer ya29.A0ARrdaM-abcdef0123456789",
        &["ya29", "abcdef0123456789"],
    );
    assert_gone("sent Bearer abc123def456 ok", &["abc123def456"]);
    assert_gone(
        "Cookie: SID=g.a000secretvalue; __Secure-3PSID=g.a000other; HSID=AbCd1234\nnext line",
        &["secretvalue", "g.a000other", "AbCd1234"],
    );
    assert_eq!(
        r().redact("Cookie: a=b\nkeep this"),
        "Cookie: [redacted]\nkeep this"
    );
    assert_gone(
        r#"headers {"cookie": "SID=zzz", "accept": "*/*"}"#,
        &["SID=zzz"],
    );
}

#[test]
fn secret_pairs_lose_their_value() {
    assert_eq!(
        r().redact("oauth_token=2-292593-abc ok"),
        "oauth_token=[redacted] ok"
    );
    assert_eq!(
        r().redact("sapisid=abc/def; other=1"),
        "sapisid=[redacted]; other=1"
    );
    assert_eq!(
        r().redact(r#"{"access_token": "xyz", "n": 1}"#),
        r#"{"access_token": "[redacted]", "n": 1}"#
    );
    assert_eq!(
        r().redact("client_id=Zx81hdkQ status=403"),
        "client_id=[redacted] status=403"
    );
    assert_eq!(r().redact("status=403 ms=41"), "status=403 ms=41");
    assert_eq!(r().redact("token:\"unclosed value"), "token:\"[redacted]");
}

#[test]
fn emails_are_replaced() {
    assert_eq!(
        r().redact("user bob.smith+x@mail.example.com signed in"),
        "user [email] signed in"
    );
    assert_eq!(r().redact("mail me at a@b.co."), "mail me at [email].");
    assert_eq!(r().redact("at @ home and a@b"), "at @ home and a@b");
}

#[test]
fn home_paths_become_a_tilde() {
    let r = r();
    assert_eq!(
        r.redact("open /home/alice/.local/share/NemoMusic/db"),
        "open ~/…"
    );
    assert_eq!(r.redact("saved in /home/alice."), "saved in ~.");
    assert_eq!(r.redact("kept /home/alice2/x"), "kept ~/…");
    assert_eq!(
        r.redact("in /home/bob/x and /Users/carol/y"),
        "in ~/… and ~/…"
    );
    assert_eq!(
        r.redact(r"C:\Users\Dave\AppData\x and D:/users/eve/z"),
        "~\\… and ~/…"
    );
    assert_eq!(r.redact("cwd /root/work"), "cwd ~/…");
    assert_eq!(
        r.redact("PATH=/usr/bin:/home/alice/bin"),
        "PATH=/usr/bin:~/…"
    );
    assert_eq!(
        r.redact("keep /var/home/x and ~/home/y"),
        "keep /var/home/x and ~/…"
    );
    let win = Redactor::with_home(r"C:\Users\Alice");
    assert_eq!(win.redact(r"C:\Users\Alice\Music"), "~\\…");
    assert_eq!(win.redact("C:/Users/Alice/Music"), "~/…");
}

#[test]
fn a_tiny_home_is_ignored() {
    assert_eq!(Redactor::with_home("/").redact("a/b/c"), "a/b/c");
    assert_eq!(Redactor::with_home("").redact("a/b/c"), "a/b/c");
}

#[test]
fn long_id_like_tokens_are_replaced() {
    let r = r();
    assert_eq!(
        r.redact("playlist PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf ok"),
        "playlist [id] ok"
    );
    assert_eq!(
        r.redact("channel UCuAXFkgsw1L7xaCfnd5JJOw end"),
        "channel [id] end"
    );
    assert_eq!(
        r.redact("id 123e4567-e89b-12d3-a456-426614174000"),
        "id [id]"
    );
    assert_eq!(r.redact("0123456789abcdef0123456789abcdef"), "[id]");
    assert_eq!(
        r.redact("jwt eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.sig"),
        "jwt [id].[id].sig"
    );
    // Ordinary words and build triples stay.
    assert_eq!(
        r.redact("x86_64-unknown-linux-gnu on yt-dlp 2025.01.15"),
        "x86_64-unknown-linux-gnu on yt-dlp 2025.01.15"
    );
    assert_eq!(r.redact("dQw4w9WgXcQ"), "dQw4w9WgXcQ");
    assert_eq!(
        r.redact("a very long wordlike sentence stays readable"),
        "a very long wordlike sentence stays readable"
    );
}

#[test]
fn control_characters_become_spaces() {
    assert_eq!(r().redact("a\tb\u{7}c\r\nd"), "a b c\nd");
}

#[test]
fn nasty_strings_do_not_leak_or_panic() {
    let nasty: &[(&str, &[&str])] = &[
        ("", &[]),
        ("://", &[]),
        ("https://", &[]),
        ("https://@", &[]),
        ("https:////x//y", &["y"]),
        ("a@@b@@c.d", &[]),
        ("???===&&&", &[]),
        ("token=token=token=secret1", &["secret1"]),
        ("Bearer", &[]),
        ("bearer \u{1F600}tok", &["tok"]),
        ("cookie:", &[]),
        ("/home/", &[]),
        ("/home/alice", &["alice"]),
        ("C:\\Users\\", &[]),
        (
            "日本語 https://例え.jp/パス?キー=値 終わり",
            &["パス", "キー", "値"],
        ),
        (
            "https://a.b/c?d=e https://f.g/h?i=j",
            &["/c", "/h", "d=e", "i=j"],
        ),
        ("ftp://u:p@host.example/secret/path", &["secret", "u:p"]),
        (
            "wss://sync.example.com/room/9f8e7d6c5b4a39281706f5e4d3c2b1a0",
            &["9f8e7d6c"],
        ),
        (
            "password=hunter2 password: hunter3",
            &["hunter2", "hunter3"],
        ),
        (
            "SAPISIDHASH 1234_abcdef0123456789abcdef01234567",
            &["abcdef0123456789abcdef01234567"],
        ),
        (
            "NID=511=AbCdEfGhIjKlMnOpQrStUvWxYz0123456789",
            &["AbCdEfGhIj"],
        ),
        ("e-mail:me@example.org;x", &["me@example.org"]),
        (
            "ASDFGHJKLQWERTYUIOPZXCVBNM1234567890abcdefghij",
            &["ASDFGHJKL"],
        ),
    ];
    for (input, secrets) in nasty {
        assert_gone(input, secrets);
    }
}

#[test]
fn a_very_long_input_is_handled() {
    let long = "x ".repeat(50_000) + "https://a.b/c?d=e";
    let out = r().redact(&long);
    assert!(out.ends_with("a.b"));
}

fn token() -> impl Strategy<Value = String> {
    "[A-Za-z0-9]{32,40}"
}

proptest! {
    #[test]
    fn redaction_never_panics_and_is_idempotent(text in ".{0,200}") {
        let once = r().redact(&text);
        prop_assert_eq!(r().redact(&once), once);
    }

    #[test]
    fn redaction_is_idempotent_on_secret_shaped_text(
        text in "[a-zA-Z0-9:/?=&@.;,\"' \\\\_%#~\\[\\]()-]{0,120}"
    ) {
        let once = r().redact(&text);
        prop_assert_eq!(r().redact(&once), once);
    }

    #[test]
    fn a_planted_secret_never_survives(secret in token(), template in 0usize..9) {
        let input = match template {
            0 => format!("GET https://a.example.com/v1/{secret}/x?k={secret}"),
            1 => format!("failed (https://u:{secret}@h.example.org:8443/p)"),
            2 => format!("Authorization: Bearer {secret}"),
            3 => format!("Cookie: SID={secret}; HSID={secret}"),
            4 => format!("oauth_token={secret}&x=1"),
            5 => format!("mail {secret}@example.com sent"),
            6 => format!("video_id={secret} bad"),
            7 => format!("/home/alice/{secret}/file {secret}"),
            _ => format!("id {secret} gone"),
        };
        let out = r().redact(&input);
        prop_assert!(!out.contains(&secret), "{out}");
    }
}
