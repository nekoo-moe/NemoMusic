//! Pure helpers: which pages count as "signed in" and how cookies become one header value.
//!
//! Nothing here logs or prints. Cookie values only travel from the `WebView2` page to the stdout pipe.

/// The page that ends the sign-in flow.
pub const MUSIC_ORIGIN: &str = "https://music.youtube.com";

/// Name of the cookie that proves a signed-in session (the one `ytmapi-rs` signs requests with).
const SESSION_COOKIE: &str = "SAPISID";

/// Google's sign-in page for YouTube Music. After sign-in it continues to `music.youtube.com`.
pub const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F";

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
