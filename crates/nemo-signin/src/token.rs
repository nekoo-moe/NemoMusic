//! SoundCloud's session token as it is found in a cookie: which values count as a token, how
//! a `document.cookie` string is read, and the one-line reports of the diagnostic mode.
//!
//! Nothing here logs or prints. The reports ([`cookie_line`], [`storage_line`]) hold names,
//! flags, lengths and a shape class, never a value, so a log of them can be pasted anywhere.

/// Shortest value that counts as a token.
pub const MIN_TOKEN_BYTES: usize = 8;
/// Longest value that counts as a token. Real ones are about 40 characters; the limit leaves room
/// for a token with a suffix.
pub const MAX_TOKEN_BYTES: usize = 2048;

/// Name of SoundCloud's session cookie: its value is the token the web app sends as
/// `Authorization: OAuth <token>`.
pub const TOKEN_COOKIE: &str = "oauth_token";

/// A character that may be in a token: visible ASCII that cannot break a header or a cookie list
/// (no space, quote, comma, semicolon, backslash or angle bracket).
const fn is_token_byte(b: u8) -> bool {
    b.is_ascii_graphic() && !matches!(b, b'"' | b';' | b',' | b'\\' | b'<' | b'>')
}

/// Whether `token` looks like a SoundCloud token: [`MIN_TOKEN_BYTES`] to [`MAX_TOKEN_BYTES`] of
/// header-safe printable characters.
#[must_use]
pub fn is_token_shaped(token: &str) -> bool {
    (MIN_TOKEN_BYTES..=MAX_TOKEN_BYTES).contains(&token.len()) && token.bytes().all(is_token_byte)
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// `value` with its `%XX` escapes decoded, or `None` when it has none or a bad one, or when the
/// result is not text.
#[must_use]
pub fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut any = false;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let high = bytes.get(i + 1).copied().and_then(hex)?;
            let low = bytes.get(i + 2).copied().and_then(hex)?;
            out.push(high * 16 + low);
            i += 3;
            any = true;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    if any {
        String::from_utf8(out).ok()
    } else {
        None
    }
}

/// The token in a cookie value: the value without surrounding quotes, percent-decoded when it
/// carries escapes, and only when the result is shaped like a token.
#[must_use]
pub fn token_from_value(raw: &str) -> Option<String> {
    let value = raw.trim().trim_matches('"').trim();
    if let Some(decoded) = percent_decode(value)
        && is_token_shaped(&decoded)
    {
        return Some(decoded);
    }
    is_token_shaped(value).then(|| value.to_owned())
}

/// The token in a `document.cookie` string (`a=b; oauth_token=c; d=e`): only the cookie named
/// [`TOKEN_COOKIE`] is read, every other cookie is dropped here.
#[must_use]
pub fn token_from_document_cookie(text: &str) -> Option<String> {
    text.split(';').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        (name.trim() == TOKEN_COOKIE)
            .then(|| token_from_value(value))
            .flatten()
    })
}

/// The shape of a value, in a word that says why it is or is not a token. Never the value.
#[must_use]
pub fn shape_class(value: &str) -> &'static str {
    let inner = value.trim();
    if inner.is_empty() {
        "empty"
    } else if !inner.is_ascii() {
        "non-ascii"
    } else if inner.len() < MIN_TOKEN_BYTES {
        "too-short"
    } else if inner.len() > MAX_TOKEN_BYTES {
        "too-long"
    } else if inner.starts_with('"') {
        if token_from_value(inner).is_some() {
            "quoted-token"
        } else {
            "quoted-other"
        }
    } else if percent_decode(inner).is_some_and(|d| is_token_shaped(&d)) {
        "percent-encoded-token"
    } else if is_token_shaped(inner) {
        "token"
    } else if inner.contains(' ') {
        "has-space"
    } else {
        "has-other-characters"
    }
}

/// A cookie or storage name that is safe to write in a log: ASCII letters, digits and `-_.`, at
/// most 40 characters, and a run of six or more digits (an id) replaced by `#`.
#[must_use]
pub fn log_name(name: &str) -> String {
    let mut out = String::new();
    let mut digits = 0_usize;
    for c in name.chars().take(40) {
        if c.is_ascii_digit() {
            digits += 1;
            if digits <= 5 {
                out.push(c);
            } else if digits == 6 {
                // The first five digits were already pushed: replace the run.
                out.truncate(out.len() - 5);
                out.push('#');
            }
        } else {
            digits = 0;
            out.push(if c.is_ascii_alphabetic() || "-_.".contains(c) {
                c
            } else {
                '?'
            });
        }
    }
    out
}

/// One line about one cookie: name, domain, flags, value length and shape. No value.
#[must_use]
pub fn cookie_line(
    source: &str,
    name: &str,
    domain: Option<&str>,
    flags: (Option<bool>, Option<bool>),
    value: &str,
) -> String {
    let flag = |f: Option<bool>| f.map_or("?", |b| if b { "yes" } else { "no" });
    format!(
        "cookie source={source} name={} domain={} secure={} httponly={} length={} shape={}",
        log_name(name),
        domain.map_or_else(|| "none".to_owned(), log_name),
        flag(flags.0),
        flag(flags.1),
        value.len(),
        shape_class(value),
    )
}

/// The lines about the cookies of a `document.cookie` string: names, lengths and shapes.
#[must_use]
pub fn document_cookie_lines(text: &str) -> Vec<String> {
    text.split(';')
        .filter_map(|part| part.split_once('='))
        .map(|(name, value)| cookie_line("document", name.trim(), None, (None, None), value))
        .collect()
}

/// One line about the `Authorization` value the page itself sent: length and shape, no value.
#[must_use]
pub fn header_line(value: &str) -> String {
    format!(
        "page request header=authorization length={} shape={}",
        value.len(),
        shape_class(value)
    )
}

/// One line about a storage entry or a hydration name: a name and a length, no value.
#[must_use]
pub fn storage_line(area: &str, name: &str, length: usize) -> String {
    format!("{area} name={} length={length}", log_name(name))
}

#[cfg(test)]
mod tests;
