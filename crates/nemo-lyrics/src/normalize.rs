//! String normalization for track matching.
//!
//! Steps: Unicode NFKC (folds full-width forms, ideographic space, ligatures, compatibility
//! characters), Unicode-aware lowercase, strip `(feat. ...)` / `[feat. ...]` /
//! `(Remastered ...)` groups and `- Remastered 2011` style suffixes, drop apostrophes, turn
//! other punctuation into spaces, collapse spaces, then NFKC once more (a dropped apostrophe can
//! leave marks that compose or reorder).

use unicode_normalization::UnicodeNormalization;
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
    let lowered: String = input.chars().nfkc().flat_map(char::to_lowercase).collect();
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
    let joined = out.split_whitespace().collect::<Vec<_>>().join(" ");
    // Dropping an apostrophe can leave two combining marks side by side out of canonical
    // order (or a base and a mark that compose), so compose once more; without this a second
    // pass would change the text and `normalize` would not be idempotent.
    joined.nfkc().collect()
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
    fn nfkc_folds_compatibility_forms() {
        let cases: &[(&str, &str)] = &[
            // Full-width Latin, digits and ideographic space.
            (
                "\u{ff34}\u{ff25}\u{ff33}\u{ff34}\u{3000}\u{ff11}\u{ff12}",
                "test 12",
            ),
            // Ligature fi, superscript two, roman numeral three.
            ("\u{fb01}ne", "fine"),
            ("x\u{b2}", "x2"),
            ("\u{2162}", "iii"),
            // Half-width katakana folds to full-width katakana.
            ("\u{ff76}\u{ff85}", "\u{30ab}\u{30ca}"),
            // Precomposed and decomposed forms compare equal.
            ("caf\u{e9}", "caf\u{e9}"),
            ("cafe\u{301}", "caf\u{e9}"),
            // Full-width brackets are NFKC-folded, so the noise group is stripped.
            ("Song \u{ff08}feat. X\u{ff09}", "song"),
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
            // An apostrophe between two combining marks (found by the property test).
            "\u{816}`\u{c55}",
        ] {
            let once = normalize(s);
            assert_eq!(normalize(&once), once, "input {s:?}");
        }
    }

    #[test]
    fn dropping_an_apostrophe_keeps_marks_in_canonical_order() {
        // U+0C55 (combining class 84) sorts before U+0816 (230) once nothing sits between them.
        assert_eq!(normalize("\u{816}`\u{c55}"), "\u{c55}\u{816}");
    }
}
