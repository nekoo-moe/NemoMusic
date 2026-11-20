//! Redaction of text before it enters the developer ring or a report (dev-tools spec).
//!
//! The rules run in a fixed order and the whole pass is idempotent: redacting redacted text
//! changes nothing. It works on characters, never panics and uses no regular expressions.

mod pairs;
mod url;

#[cfg(test)]
mod tests;

/// Marker that replaces a secret value.
pub(super) const REDACTED: &str = "[redacted]";

/// Longest id-like run kept as it is: shorter runs are ordinary words and short ids.
const ID_MIN: usize = 20;

/// Runs this long are never an ordinary word.
const ID_ALWAYS: usize = 32;

/// Redacts URLs, queries, bearer and cookie text, e-mail addresses, home paths and id-like
/// tokens. Cheap to build and to share.
#[derive(Debug, Clone)]
pub struct Redactor {
    homes: Vec<String>,
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new()
    }
}

impl Redactor {
    /// A redactor that also knows this user's home folder (`HOME` and `USERPROFILE`).
    #[must_use]
    pub fn new() -> Self {
        let mut redactor = Self { homes: Vec::new() };
        for var in ["HOME", "USERPROFILE"] {
            if let Ok(home) = std::env::var(var) {
                redactor.add_home(&home);
            }
        }
        redactor
    }

    /// A redactor for one given home folder (tests).
    #[cfg(test)]
    #[must_use]
    pub fn with_home(home: &str) -> Self {
        let mut redactor = Self { homes: Vec::new() };
        redactor.add_home(home);
        redactor
    }

    fn add_home(&mut self, home: &str) {
        let home = home.trim().trim_end_matches(['/', '\\']);
        // `/` or `/x` would eat ordinary text.
        if home.chars().count() < 4 {
            return;
        }
        for variant in [
            home.to_owned(),
            home.replace('\\', "/"),
            home.replace('/', "\\"),
        ] {
            if !self.homes.contains(&variant) {
                self.homes.push(variant);
            }
        }
    }

    /// The redacted text. Line breaks stay; other control characters become spaces.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        let text = pairs::headers(&clean(text));
        let text = url::strip_urls(&text);
        let text = pairs::bearer(&text);
        let text = pairs::secret_pairs(&text);
        let text = url::strip_queries(&text);
        let text = emails(&text);
        let text = self.home_paths(&text);
        ids(&text)
    }

    fn home_paths(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for home in &self.homes {
            text = replace_bounded(&text, home, "~");
        }
        url::generic_home_paths(&text)
    }
}

/// Drop carriage returns and turn control characters (tabs too) into spaces.
fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| *c != '\r')
        .map(|c| if c != '\n' && c.is_control() { ' ' } else { c })
        .collect()
}

/// Replace `needle` by `with` where the next character does not continue a folder name.
fn replace_bounded(text: &str, needle: &str, with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(needle) {
        let (before, after) = rest.split_at(at);
        out.push_str(before);
        let tail = &after[needle.len()..];
        let mut next = tail.chars();
        let continues = match next.next() {
            Some(c) if c.is_alphanumeric() || matches!(c, '_' | '-') => true,
            // A dot ends a sentence unless a name goes on after it (`folder.bak`).
            Some('.') => next.next().is_some_and(char::is_alphanumeric),
            _ => false,
        };
        out.push_str(if continues { needle } else { with });
        rest = tail;
    }
    out.push_str(rest);
    out
}

fn is_local_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-')
}

fn is_domain_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-')
}

/// Replace `name@domain.tld` by `[email]`.
fn emails(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '@' {
            let local = out.iter().rev().take_while(|c| is_local_char(**c)).count();
            let mut end = i + 1;
            while end < chars.len() && is_domain_char(chars[end]) {
                end += 1;
            }
            // A sentence-ending dot is not part of the address.
            while end > i + 1 && chars[end - 1] == '.' {
                end -= 1;
            }
            let domain = &chars[i + 1..end];
            if local > 0 && domain_ok(domain) {
                out.truncate(out.len() - local);
                out.extend("[email]".chars());
                i = end;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out.into_iter().collect()
}

fn domain_ok(domain: &[char]) -> bool {
    let Some(last_dot) = domain.iter().rposition(|c| *c == '.') else {
        return false;
    };
    last_dot > 0 && domain.len() - last_dot > 2
}

fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-')
}

/// Replace id-like runs by `[id]`.
fn ids(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    for c in text.chars() {
        if is_id_char(c) {
            run.push(c);
        } else {
            flush_run(&mut run, &mut out);
            out.push(c);
        }
    }
    flush_run(&mut run, &mut out);
    out
}

fn flush_run(run: &mut String, out: &mut String) {
    out.push_str(if looks_like_id(run) { "[id]" } else { run });
    run.clear();
}

/// A long run of letters, digits, `_` and `-`: a video, playlist or channel id, a token or a
/// hash. Words with separators and no capital letters (`x86_64-unknown-linux-gnu`) stay.
fn looks_like_id(run: &str) -> bool {
    let len = run.len();
    if len >= ID_ALWAYS {
        return true;
    }
    if len < ID_MIN {
        return false;
    }
    let digit = run.bytes().any(|b| b.is_ascii_digit());
    let letter = run.bytes().any(|b| b.is_ascii_alphabetic());
    let upper = run.bytes().any(|b| b.is_ascii_uppercase());
    let separator = run.bytes().any(|b| matches!(b, b'_' | b'-'));
    digit && letter && (upper || !separator)
}
