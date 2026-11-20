//! Pure helpers: which pages count as "signed in" and how cookies become one header value or one
//! token.
//!
//! Nothing here logs or prints. Cookie values only travel from the `WebView2` page to the stdout pipe.

use crate::token::{TOKEN_COOKIE, token_from_value};

/// The page that ends the sign-in flow.
pub const MUSIC_ORIGIN: &str = "https://music.youtube.com";

/// Name of the cookie that proves a signed-in session (the one `ytmapi-rs` signs requests with).
const SESSION_COOKIE: &str = "SAPISID";

/// Google's sign-in page for YouTube Music. After sign-in it continues to `music.youtube.com`.
pub const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F";

/// The SoundCloud page that starts the sign-in.
pub const SOUNDCLOUD_SIGN_IN_URL: &str = "https://soundcloud.com/signin";

/// The origin whose cookies are read in SoundCloud mode.
pub const SOUNDCLOUD_ORIGIN: &str = "https://soundcloud.com";

/// True when `url` is a page of `soundcloud.com` or `www.soundcloud.com` (any path, query or
/// fragment).
#[must_use]
pub fn is_soundcloud_page(url: &str) -> bool {
    ["https://soundcloud.com", "https://www.soundcloud.com"]
        .iter()
        .any(|origin| {
            url.strip_prefix(origin)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '?', '#']))
        })
}

/// A cookie domain that belongs to `soundcloud.com` (`soundcloud.com`, `.soundcloud.com`, or a
/// host below it). `None` (the webview gave no domain) passes: the cookie was read for the
/// `soundcloud.com` address.
fn is_soundcloud_domain(domain: Option<&str>) -> bool {
    domain.is_none_or(|d| {
        let d = d.trim_start_matches('.');
        d == "soundcloud.com" || d.ends_with(".soundcloud.com")
    })
}

/// The value of the `oauth_token` cookie of `soundcloud.com`, and nothing else.
///
/// Each cookie is `(name, value, domain)`. Other cookies, other domains and values that are not
/// shaped like a token give `None`, so nothing but the session token leaves the window.
#[must_use]
pub fn soundcloud_token<'a>(
    cookies: impl IntoIterator<Item = (&'a str, &'a str, Option<&'a str>)>,
) -> Option<String> {
    cookies.into_iter().find_map(|(name, value, domain)| {
        (name == TOKEN_COOKIE && is_soundcloud_domain(domain))
            .then(|| token_from_value(value))
            .flatten()
    })
}

/// The one stdout line that carries a SoundCloud token.
#[must_use]
pub fn token_line(token: &str) -> String {
    serde_json::json!({ "token": token }).to_string()
}

/// True when `url` is a page of `music.youtube.com` (any path, query or fragment).
#[must_use]
pub fn is_music_page(url: &str) -> bool {
    url.strip_prefix(MUSIC_ORIGIN)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '?', '#']))
}

/// Only `https` pages and `about:blank` may load in the sign-in window.
#[must_use]
pub fn is_allowed_navigation(url: &str) -> bool {
    url.starts_with("https://") || url == "about:blank"
}

fn is_header_safe(text: &str) -> bool {
    !text.chars().any(|c| c.is_control() || c == ';')
}

/// Join cookies into a `Cookie` header value (`a=b; c=d`).
///
/// Returns `None` until the session cookie `SAPISID` is present. Cookies that would break the
/// header (control characters or `;`) and cookies without a name are skipped.
#[must_use]
pub fn cookie_header<'a>(cookies: impl IntoIterator<Item = (&'a str, &'a str)>) -> Option<String> {
    let mut parts = Vec::new();
    let mut has_session = false;
    for (name, value) in cookies {
        if name.is_empty() || !is_header_safe(name) || !is_header_safe(value) {
            continue;
        }
        if name == SESSION_COOKIE && !value.is_empty() {
            has_session = true;
        }
        parts.push(format!("{name}={value}"));
    }
    has_session.then(|| parts.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_pages_are_recognised() {
        for url in [
            "https://music.youtube.com",
            "https://music.youtube.com/",
            "https://music.youtube.com/?cbrd=1",
            "https://music.youtube.com#x",
        ] {
            assert!(is_music_page(url), "{url}");
        }
        for url in [
            "https://music.youtube.com.evil.example/",
            "https://accounts.google.com/?continue=https://music.youtube.com/",
            "http://music.youtube.com/",
            "https://www.youtube.com/signin",
            "",
        ] {
            assert!(!is_music_page(url), "{url}");
        }
    }

    fn jar<'a>(
        list: &'a [(&'a str, &'a str, Option<&'a str>)],
    ) -> impl Iterator<Item = (&'a str, &'a str, Option<&'a str>)> {
        list.iter().copied()
    }

    #[test]
    fn soundcloud_pages_are_recognised() {
        for url in [
            "https://soundcloud.com",
            "https://soundcloud.com/signin",
            "https://www.soundcloud.com/discover?x=1",
            "https://soundcloud.com#x",
        ] {
            assert!(is_soundcloud_page(url), "{url}");
        }
        for url in [
            "https://soundcloud.com.evil.example/",
            "https://evil.example/?u=https://soundcloud.com/",
            "http://soundcloud.com/",
            "https://secure.soundcloud.com/authorize",
            "https://notsoundcloud.com/",
            "",
        ] {
            assert!(!is_soundcloud_page(url), "{url}");
        }
    }

    #[test]
    fn only_the_oauth_token_of_soundcloud_is_returned() {
        let token = "2-293232-1234567-AbCdEfGh";
        let list = [
            ("sc_anonymous_id", "111111111", Some(".soundcloud.com")),
            ("oauth_token", token, Some(".soundcloud.com")),
            ("other", "2-293232-1234567-zzzzzzzz", Some("soundcloud.com")),
        ];
        assert_eq!(soundcloud_token(jar(&list)).as_deref(), Some(token));
        let no_domain = [("oauth_token", token, None)];
        assert_eq!(soundcloud_token(jar(&no_domain)).as_deref(), Some(token));
        let below = [("oauth_token", token, Some("api.soundcloud.com"))];
        assert_eq!(soundcloud_token(jar(&below)).as_deref(), Some(token));
    }

    #[test]
    fn a_token_from_another_domain_or_of_the_wrong_shape_is_not_returned() {
        let token = "2-293232-1234567-AbCdEfGh";
        for list in [
            vec![("oauth_token", token, Some(".evil-soundcloud.com"))],
            vec![("oauth_token", token, Some("soundcloud.com.evil.example"))],
            vec![("oauth_token", "", Some(".soundcloud.com"))],
            vec![("oauth_token", "short", Some(".soundcloud.com"))],
            vec![("oauth_token", "has space in it", Some(".soundcloud.com"))],
            vec![("oauth_token", "a;b=c-1234567", Some(".soundcloud.com"))],
            vec![("oauth_token\n", token, Some(".soundcloud.com"))],
            vec![("sc_anonymous_id", token, Some(".soundcloud.com"))],
        ] {
            assert_eq!(soundcloud_token(list.iter().copied()), None, "{list:?}");
        }
        let long = "a".repeat(crate::token::MAX_TOKEN_BYTES + 1);
        let list = [("oauth_token", long.as_str(), None)];
        assert_eq!(soundcloud_token(jar(&list)), None);
    }

    #[test]
    fn the_token_line_is_one_json_object_with_only_the_token() {
        let line = token_line("2-293232-1234567-AbCdEfGh");
        assert_eq!(line, r#"{"token":"2-293232-1234567-AbCdEfGh"}"#);
        assert!(!line.contains('\n'));
    }

    #[test]
    fn only_https_may_load() {
        assert!(is_allowed_navigation("https://accounts.google.com/"));
        assert!(is_allowed_navigation("about:blank"));
        assert!(!is_allowed_navigation("http://music.youtube.com/"));
        assert!(!is_allowed_navigation("file:///C:/x"));
        assert!(!is_allowed_navigation("javascript:alert(1)"));
    }

    #[test]
    fn header_needs_the_session_cookie() {
        assert_eq!(cookie_header([("a", "1"), ("b", "2")]), None);
        assert_eq!(cookie_header([("SAPISID", "")]), None);
        assert_eq!(
            cookie_header([("a", "1"), ("SAPISID", "x"), ("c", "3")]).as_deref(),
            Some("a=1; SAPISID=x; c=3")
        );
    }

    #[test]
    fn unsafe_cookies_are_skipped() {
        let header = cookie_header([
            ("SAPISID", "x"),
            ("bad;name", "1"),
            ("bad", "a;b"),
            ("line", "a\nb"),
            ("", "v"),
            ("ok", "\"quoted\""),
        ]);
        assert_eq!(header.as_deref(), Some("SAPISID=x; ok=\"quoted\""));
    }
}
