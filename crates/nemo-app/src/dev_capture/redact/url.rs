//! URLs, query strings and home-folder paths.

/// Characters that end a URL in running text.
fn ends_url(c: char) -> bool {
    c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | ')' | ']' | '}' | '|' | '`')
}

/// Characters that end a query string. `]` does not: `[redacted]` can stand inside one.
fn ends_query(c: char) -> bool {
    c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | ')' | '|' | '`')
}

fn is_scheme_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')
}

/// Replace every `scheme://...` by its host: no scheme, user info, port, path, query or
/// fragment. `file://` becomes `[path]` and an odd host becomes `[url]`.
pub(super) fn strip_urls(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ':' && chars.get(i + 1) == Some(&'/') && chars.get(i + 2) == Some(&'/') {
            let scheme_len = out.iter().rev().take_while(|c| is_scheme_char(**c)).count();
            let scheme: String = out[out.len() - scheme_len..].iter().collect();
            let end = url_end(&chars, i + 3);
            out.truncate(out.len() - scheme_len);
            out.extend(url_host(&scheme, &chars[i + 3..end]).chars());
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out.into_iter().collect()
}

/// Where the URL whose authority starts at `from` ends. A bracketed IPv6 host may hold `]`.
fn url_end(chars: &[char], from: usize) -> usize {
    let mut end = from;
    if chars.get(end) == Some(&'[') {
        while end < chars.len() && chars[end] != ']' && !chars[end].is_whitespace() {
            end += 1;
        }
        if chars.get(end) == Some(&']') {
            end += 1;
        }
    }
    while end < chars.len() && !ends_url(chars[end]) {
        end += 1;
    }
    end
}

/// The text that stands for a URL: its host, with trailing sentence punctuation kept.
fn url_host(scheme: &str, rest: &[char]) -> String {
    if scheme.eq_ignore_ascii_case("file") {
        return "[path]".to_owned();
    }
    let authority_end = rest
        .iter()
        .position(|c| matches!(c, '/' | '?' | '#'))
        .unwrap_or(rest.len());
    let has_path = authority_end < rest.len();
    let authority: String = rest[..authority_end].iter().collect();
    let host = authority
        .rsplit_once('@')
        .map_or(authority.as_str(), |(_, h)| h);
    let trimmed = host.trim_end_matches(['.', ',', ';', ':', '!']);
    let punctuation = if has_path { "" } else { &host[trimmed.len()..] };
    let host = strip_port(trimmed);
    if is_plain_host(host) {
        format!("{host}{punctuation}")
    } else {
        format!("[url]{punctuation}")
    }
}

fn strip_port(host: &str) -> &str {
    if host.starts_with('[') {
        return host.find(']').map_or(host, |end| &host[..=end]);
    }
    match host.rsplit_once(':') {
        Some((name, port)) if port.chars().all(|c| c.is_ascii_digit()) => name,
        _ => host,
    }
}

fn is_plain_host(host: &str) -> bool {
    if let Some(v6) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return !v6.is_empty()
            && v6
                .chars()
                .all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '.'));
    }
    !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Replace a query string that is left without its URL (`/path?a=b&c=d`) by `?[query]`.
pub(super) fn strip_queries(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        out.push(chars[i]);
        let starts_query = chars[i] == '?'
            && chars
                .get(i + 1)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_');
        i += 1;
        if starts_query {
            let mut end = i;
            while end < chars.len() && !ends_query(chars[end]) {
                end += 1;
            }
            if chars[i..end].contains(&'=') {
                out.push_str("[query]");
                i = end;
            }
        }
    }
    out
}

/// Replace `/home/<name>`, `/Users/<name>`, `/root` and `X:\Users\<name>` by `~`.
pub(super) fn generic_home_paths(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if let Some(end) = home_prefix_end(&chars, i, &out) {
            out.push('~');
            i = end;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// If a home prefix starts at `i`, the index after its user name.
fn home_prefix_end(chars: &[char], i: usize, before: &str) -> Option<usize> {
    let starts_path = |c: Option<char>| {
        c.is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '/' | '\\' | '.' | '~' | '_' | '-')))
    };
    if !starts_path(before.chars().last()) {
        return None;
    }
    if let Some(after_drive) = windows_users_end(chars, i) {
        return Some(user_name_end(chars, after_drive));
    }
    for marker in ["/home/", "/Users/"] {
        if starts_with_at(chars, i, marker) {
            return Some(user_name_end(chars, i + marker.len()));
        }
    }
    if starts_with_at(chars, i, "/root") {
        let end = i + "/root".len();
        let continues = chars
            .get(end)
            .is_some_and(|c| c.is_alphanumeric() || *c == '_');
        return (!continues).then_some(end);
    }
    None
}

/// `C:\Users\` or `C:/Users/` at `i`: the index just after the separator that follows `Users`.
fn windows_users_end(chars: &[char], i: usize) -> Option<usize> {
    let drive = chars.get(i)?;
    if !drive.is_ascii_alphabetic() || chars.get(i + 1) != Some(&':') {
        return None;
    }
    if !matches!(chars.get(i + 2), Some('\\' | '/')) {
        return None;
    }
    let name: String = chars.get(i + 3..i + 8)?.iter().collect();
    if !name.eq_ignore_ascii_case("users") || !matches!(chars.get(i + 8), Some('\\' | '/')) {
        return None;
    }
    Some(i + 9)
}

fn user_name_end(chars: &[char], mut at: usize) -> usize {
    while at < chars.len()
        && !(chars[at].is_whitespace()
            || matches!(
                chars[at],
                '/' | '\\' | ':' | '"' | '\'' | ')' | ']' | '>' | ',' | ';'
            ))
    {
        at += 1;
    }
    at
}

fn starts_with_at(chars: &[char], at: usize, text: &str) -> bool {
    text.chars()
        .enumerate()
        .all(|(k, c)| chars.get(at + k) == Some(&c))
}
