//! String normalization for track matching.
//!
//! Std-only. This is NOT full Unicode NFKC (that needs the `unicode-normalization` crate, which
//! is not approved). What it does: lowercase (Unicode aware), fold full-width ASCII forms
//! (U+FF01..U+FF5E) and the ideographic space to ASCII, drop apostrophes, turn other
//! punctuation into spaces, strip `(feat. ...)` / `[feat. ...]` / `(Remastered ...)` groups and
//! `- Remastered 2011` style suffixes, collapse spaces.

/// Fold a char toward its ASCII form (partial NFKC: full-width forms only).
fn fold_width(c: char) -> char {
    match c {
        '\u{3000}' => ' ',
        '\u{ff01}'..='\u{ff5e}' => char::from_u32(u32::from(c) - 0xfee0).unwrap_or(c),
        _ => c,
    }
}

/// True if a bracket group's lowercase content is noise to drop.
fn is_noise_group(content: &str) -> bool {
    let c = content.trim_start();
    let starts_with_word = |w: &str| {
        c.strip_prefix(w)
            .is_some_and(|r| r.chars().next().is_none_or(|n| !n.is_alphanumeric()))
    };
    starts_with_word("feat")
        || starts_with_word("ft")
        || starts_with_word("featuring")
        || c.contains("remaster")
}

/// Remove `(...)` and `[...]` groups that are noise (feat., remaster).
fn strip_groups(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        let close = match c {
            '(' => Some(')'),
            '[' => Some(']'),
            _ => None,
        };
        if let Some(close) = close
            && let Some(len) = chars[i + 1..].iter().position(|&x| x == close)
        {
            let content: String = chars[i + 1..i + 1 + len].iter().collect();
            if is_noise_group(&content) {
                out.push(' ');
                i += len + 2;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Cut a ` - Remastered 2011` / ` - feat. x` style suffix.
fn strip_dash_suffix(s: &str) -> &str {
    let mut from = 0;
    while let Some(pos) = s[from..].find(" - ") {
        let at = from + pos;
        let tail = &s[at + 3..];
        if is_noise_group(tail) {
            return &s[..at];
        }
        from = at + 3;
    }
    s
}

/// Normalize a title, artist or album for comparison.
///
/// Idempotent: `normalize(&normalize(x)) == normalize(x)`.
#[must_use]
pub fn normalize(input: &str) -> String {
    let lowered: String = input
        .chars()
        .map(fold_width)
        .flat_map(char::to_lowercase)
        .collect();
    let stripped = strip_groups(&lowered);
    let stripped = strip_dash_suffix(&stripped);
    let mut out = String::with_capacity(stripped.len());
    for c in stripped.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if matches!(c, '\'' | '\u{2019}' | '\u{2018}' | '`') {
            // "don't" -> "dont"
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn table() {
        let cases: &[(&str, &str)] = &[
            ("Test Song", "test song"),
            ("  Test   Song  ", "test song"),
            ("Test Song (feat. Some Artist)", "test song"),
            ("Test Song [feat. Some Artist]", "test song"),
            ("Test Song (Feat Some Artist) extra", "test song extra"),
            ("Test Song (ft. Some Artist)", "test song"),
            ("Test Song (featuring X)", "test song"),
            ("Test Song - Remastered 2011", "test song"),
            ("Test Song - 2011 Remaster", "test song"),
            ("Test Song (Remastered 2011)", "test song"),
            ("Test Song - Live", "test song live"),
            ("Test Song (Live)", "test song live"),
            ("Test-Song!", "test song"),
            ("Don't Stop", "dont stop"),
            ("Don\u{2019}t Stop", "dont stop"),
            ("ＴＥＳＴ　ＳＯＮＧ", "test song"),
            ("Ünï Çode", "ünï çode"),
            ("(unclosed feat. x", "unclosed feat x"),
            ("", ""),
            ("!!!", ""),
            ("Fate (Fitting)", "fate fitting"),
        ];
        for (input, want) in cases {
            assert_eq!(normalize(input), *want, "input {input:?}");
        }
    }

    #[test]
    fn is_idempotent() {
        for s in [
            "Test Song (feat. X) - Remastered 2011",
            "A & B",
            "ＴＥＳＴ",
            "İstanbul",
        ] {
            let once = normalize(s);
            assert_eq!(normalize(&once), once, "input {s:?}");
        }
    }
}
