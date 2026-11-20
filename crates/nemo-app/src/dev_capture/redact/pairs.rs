//! Header lines, bearer tokens and `key=value` secrets.

use super::REDACTED;

/// Header names whose value is dropped to the end of the line.
const HEADERS: [&str; 6] = [
    "proxy-authorization",
    "authorization",
    "set-cookie",
    "cookie",
    "x-goog-authuser",
    "x-goog-visitor-id",
];

/// Keys that are secrets or ids by their whole name.
const EXACT_KEYS: [&str; 14] = [
    "sid",
    "ssid",
    "hsid",
    "apisid",
    "sapisid",
    "psid",
    "lsid",
    "nid",
    "ysc",
    "sp_dc",
    "sig",
    "key",
    "sidcc",
    "login_info",
];

/// Keys that are secrets or ids by their start (cookie families).
const KEY_PREFIXES: [&str; 3] = ["__secure-", "__host-", "visitor_info"];

/// Keys that are secrets or ids when they hold one of these words.
const KEY_PARTS: [&str; 24] = [
    "token",
    "secret",
    "passw",
    "cookie",
    "auth",
    "session",
    "credential",
    "signature",
    "apikey",
    "api_key",
    "api-key",
    "visitor",
    "bearer",
    "oauth",
    "jwt",
    "private",
    "clientid",
    "videoid",
    "playlistid",
    "channelid",
    "browseid",
    "trackid",
    "userid",
    "accountid",
];

/// `Cookie: ...`, `Authorization: ...` and similar: the rest of the line becomes `[redacted]`.
pub(super) fn headers(text: &str) -> String {
    text.split('\n')
        .map(header_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn header_line(line: &str) -> String {
    // ASCII lowercasing keeps every byte offset valid for `line`.
    let lower = line.to_ascii_lowercase();
    let mut earliest: Option<usize> = None;
    for name in HEADERS {
        let mut from = 0;
        while let Some(at) = lower[from..].find(name) {
            let start = from + at;
            from = start + name.len();
            let inside_word = start > 0
                && matches!(lower.as_bytes()[start - 1], b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_');
            if inside_word {
                continue;
            }
            if let Some(colon) = colon_after(&lower[from..]) {
                let end = from + colon + 1;
                earliest = Some(earliest.map_or(end, |e| e.min(end)));
            }
        }
    }
    match earliest {
        Some(end) => format!("{} {REDACTED}", &line[..end]),
        None => line.to_owned(),
    }
}

/// Offset of the `:` that follows a header name, past an optional quote and spaces.
fn colon_after(rest: &str) -> Option<usize> {
    let mut offset = 0;
    let mut chars = rest.chars().peekable();
    if matches!(chars.peek(), Some('"' | '\'')) {
        chars.next();
        offset += 1;
    }
    for c in chars {
        match c {
            ' ' => offset += 1,
            ':' => return Some(offset),
            _ => return None,
        }
    }
    None
}

/// `Bearer <token>` becomes `Bearer [redacted]`.
pub(super) fn bearer(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    while let Some(at) = lower[pos..].find("bearer ") {
        let start = pos + at;
        let after = start + "bearer ".len();
        out.push_str(&text[pos..after]);
        pos = after;
        if start > 0 && lower.as_bytes()[start - 1].is_ascii_alphanumeric() {
            continue;
        }
        let rest = &text[after..];
        let token = rest.trim_start_matches(' ');
        out.push_str(&rest[..rest.len() - token.len()]);
        let token_len = token.find(char::is_whitespace).unwrap_or(token.len());
        out.push_str(REDACTED);
        pos = after + (rest.len() - token.len()) + token_len;
    }
    out.push_str(&text[pos..]);
    out
}

fn is_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')
}

fn is_value_end(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            ',' | ';' | '&' | ')' | '}' | '"' | '\'' | '<' | '>' | '|'
        )
}

/// Whether a key names a secret or an id, so its value must not be kept.
fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    EXACT_KEYS.contains(&key.as_str())
        || KEY_PREFIXES.iter().any(|p| key.starts_with(p))
        || KEY_PARTS.iter().any(|p| key.contains(p))
        || key.ends_with("_id")
        || key.ends_with("-id")
}

/// `key=value`, `key: value` and `"key": "value"` with a secret key: the value becomes
/// `[redacted]`.
pub(super) fn secret_pairs(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if !is_key_char(chars[i]) {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_key_char(chars[i]) {
            i += 1;
        }
        let key: String = chars[start..i].iter().collect();
        out.push_str(&key);
        if let Some(value_at) = value_start(&chars, i).filter(|_| is_secret_key(&key)) {
            out.extend(&chars[i..value_at]);
            i = skip_value(&chars, value_at, &mut out);
        }
    }
    out
}

/// Where the value starts after a key that ends at `i`, when a `=` or `:` follows.
fn value_start(chars: &[char], i: usize) -> Option<usize> {
    let mut j = i;
    if matches!(chars.get(j), Some('"' | '\'')) {
        j += 1;
    }
    while chars.get(j) == Some(&' ') {
        j += 1;
    }
    if !matches!(chars.get(j), Some('=' | ':')) {
        return None;
    }
    j += 1;
    while chars.get(j) == Some(&' ') {
        j += 1;
    }
    // `key://` is a URL scheme, not a pair.
    if chars.get(j) == Some(&'/') && chars.get(j + 1) == Some(&'/') {
        return None;
    }
    Some(j)
}

/// Write `[redacted]` for the value that starts at `j` and return where the text goes on.
fn skip_value(chars: &[char], j: usize, out: &mut String) -> usize {
    let mut end = j;
    if let Some(&quote @ ('"' | '\'')) = chars.get(j) {
        out.push(quote);
        end = j + 1;
        while end < chars.len() && chars[end] != quote {
            end += 1;
        }
    } else {
        while end < chars.len() && !is_value_end(chars[end]) {
            end += 1;
        }
    }
    out.push_str(REDACTED);
    end
}
