//! Token rules, the `document.cookie` reader and the diagnostic lines. The tokens are invented.

use super::*;

const TOKEN: &str = "2-293232-1234567-AbCdEfGh";

#[test]
fn a_token_is_header_safe_printable_text_of_a_sane_length() {
    assert!(is_token_shaped(TOKEN));
    assert!(
        is_token_shaped("a:b!c*d'e(f)g@h$i&j"),
        "other printable characters pass"
    );
    assert!(is_token_shaped(&"a".repeat(MAX_TOKEN_BYTES)));
    for bad in [
        String::new(),
        "short".to_owned(),
        "has space in it".to_owned(),
        "semi;colon-1234".to_owned(),
        "com,ma-1234567".to_owned(),
        "quo\"te-1234567".to_owned(),
        "back\\slash-1234".to_owned(),
        "line\nbreak-1234567".to_owned(),
        "ключ-1234567890".to_owned(),
        "a".repeat(MAX_TOKEN_BYTES + 1),
    ] {
        assert!(!is_token_shaped(&bad), "{bad:?}");
    }
}

#[test]
fn percent_escapes_are_decoded_only_when_they_are_whole_and_give_text() {
    assert_eq!(percent_decode("a%2Fb%3Dc").as_deref(), Some("a/b=c"));
    assert_eq!(percent_decode("a%2fb").as_deref(), Some("a/b"));
    assert_eq!(percent_decode("plain"), None, "nothing to decode");
    assert_eq!(percent_decode("bad%2"), None);
    assert_eq!(percent_decode("bad%zz1"), None);
    assert_eq!(percent_decode("%ff%fe"), None, "not text");
}

#[test]
fn a_cookie_value_gives_the_token_unquoted_and_decoded() {
    for (raw, want) in [
        (TOKEN.to_owned(), TOKEN),
        (format!("\"{TOKEN}\""), TOKEN),
        (format!("  {TOKEN} "), TOKEN),
        (
            "2-293232-1234567-Ab%2FCdEf%3D%3D".to_owned(),
            "2-293232-1234567-Ab/CdEf==",
        ),
        (
            "\"2-293232-1234567-Ab%2FCdEf\"".to_owned(),
            "2-293232-1234567-Ab/CdEf",
        ),
    ] {
        assert_eq!(token_from_value(&raw).as_deref(), Some(want), "{raw}");
    }
    // An escape that decodes to something unsafe keeps the value as it is.
    assert_eq!(
        token_from_value("2-293232-1234567-Ab%20Cd"),
        Some("2-293232-1234567-Ab%20Cd".to_owned())
    );
    assert_eq!(
        token_from_value("2-29%3B2-1234567-Ab%3BCd"),
        Some("2-29%3B2-1234567-Ab%3BCd".to_owned())
    );
    for bad in ["", "short", "\"\"", "a b c d e f g h"] {
        assert_eq!(token_from_value(bad), None, "{bad:?}");
    }
}

#[test]
fn document_cookie_gives_only_the_session_token() {
    let doc = format!("sc_anonymous_id=111111111; oauth_token={TOKEN}; other=abcdefghijkl");
    assert_eq!(token_from_document_cookie(&doc).as_deref(), Some(TOKEN));
    // Quoted and escaped values, spaces around the pairs.
    let quoted = format!("a=b;  oauth_token=\"{TOKEN}\" ;c=d");
    assert_eq!(token_from_document_cookie(&quoted).as_deref(), Some(TOKEN));
    let escaped = "x=1; oauth_token=2-293232-1234567-Ab%2FCdEf; y=2";
    assert_eq!(
        token_from_document_cookie(escaped).as_deref(),
        Some("2-293232-1234567-Ab/CdEf")
    );
    // The first cookie of that name that is a token wins; an empty one is passed over.
    let two = format!("oauth_token=; oauth_token={TOKEN}");
    assert_eq!(token_from_document_cookie(&two).as_deref(), Some(TOKEN));
}

#[test]
fn document_cookie_without_the_cookie_or_with_a_look_alike_gives_nothing() {
    for doc in [
        "",
        "a=b; c=d",
        "oauth_token_old=2-293232-1234567-AbCdEfGh",
        "xoauth_token=2-293232-1234567-AbCdEfGh",
        "oauth_token",
        "oauth_token=short",
        "other=oauth_token=2-293232-1234567-AbCdEfGh",
    ] {
        assert_eq!(token_from_document_cookie(doc), None, "{doc:?}");
    }
}

#[test]
fn shape_classes_say_why_without_the_value() {
    assert_eq!(shape_class(""), "empty");
    assert_eq!(shape_class("abc"), "too-short");
    assert_eq!(shape_class(&"a".repeat(MAX_TOKEN_BYTES + 1)), "too-long");
    assert_eq!(shape_class(TOKEN), "token");
    assert_eq!(shape_class(&format!("\"{TOKEN}\"")), "quoted-token");
    assert_eq!(shape_class("\"has space inside\""), "quoted-other");
    assert_eq!(
        shape_class("2-293232-1234567-Ab%2FCdEf"),
        "percent-encoded-token"
    );
    assert_eq!(shape_class("two words in it"), "has-space");
    assert_eq!(shape_class("semi;colon;value"), "has-other-characters");
    assert_eq!(shape_class("ключ-1234567890"), "non-ascii");
}

#[test]
fn log_names_hold_no_id_and_no_odd_characters() {
    assert_eq!(log_name("oauth_token"), "oauth_token");
    assert_eq!(log_name("sc_user_123456789"), "sc_user_#");
    assert_eq!(log_name("k12345"), "k12345", "five digits are not an id");
    assert_eq!(log_name("a b/c"), "a?b?c");
    assert_eq!(log_name(&"x".repeat(100)).len(), 40);
}

#[test]
fn the_lines_of_the_diagnostic_never_hold_a_value() {
    let secret = "2-293232-1234567-VerySecretValue";
    let lines = [
        cookie_line(
            "api",
            "oauth_token",
            Some(".soundcloud.com"),
            (Some(true), Some(false)),
            secret,
        ),
        cookie_line("document", "oauth_token", None, (None, None), secret),
        storage_line("localStorage", "token_user_99999999", secret.len()),
    ];
    for line in &lines {
        assert!(
            !line.contains("VerySecret") && !line.contains("2-293232"),
            "{line}"
        );
    }
    assert_eq!(
        lines[0],
        "cookie source=api name=oauth_token domain=.soundcloud.com secure=yes httponly=no length=32 shape=token"
    );
    assert!(lines[1].contains("domain=none") && lines[1].contains("secure=? httponly=?"));
    assert_eq!(lines[2], "localStorage name=token_user_# length=32");
    let header = header_line(secret);
    assert_eq!(
        header,
        "page request header=authorization length=32 shape=token"
    );
    let doc = document_cookie_lines(&format!("a=b; oauth_token={secret}"));
    assert_eq!(doc.len(), 2);
    assert!(doc.iter().all(|l| !l.contains("VerySecret")));
    assert!(doc[1].contains("name=oauth_token") && doc[1].contains("length=32"));
}
