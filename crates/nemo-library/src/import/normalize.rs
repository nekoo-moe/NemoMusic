//! Text normalization for matching imported songs against catalog results.
//!
//! Steps: Unicode NFKC (folds full-width forms and compatibility characters), lower case,
//! every `(...)` and `[...]` group removed, a ` - ` part that names a remaster or a guest
//! removed, a trailing "feat." part removed, the words "remaster" and "remastered" dropped,
//! and the rest split into words of letters and digits. File text is untrusted; nothing here
//! is executed or logged.

use unicode_normalization::UnicodeNormalization;

/// Words that start a guest part ("Song feat. X", "Artist ft. Y").
const FEAT: [&str; 3] = ["feat", "ft", "featuring"];

/// Words that tag a remaster rather than name the song.
const REMASTER: [&str; 3] = ["remaster", "remastered", "remastering"];

/// Drop `(...)` and `[...]` groups. An unclosed bracket keeps the rest of the text.
fn without_brackets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut close: Option<char> = None;
    let mut held = String::new();
    for c in text.chars() {
        match (close, c) {
            (None, '(') => close = Some(')'),
            (None, '[') => close = Some(']'),
            (None, _) => out.push(c),
            (Some(end), _) if c == end => {
                close = None;
                held.clear();
                out.push(' ');
            }
            (Some(_), _) => held.push(c),
        }
    }
    if close.is_some() {
        out.push_str(&held);
    }
    out
}

/// The words of `text`: runs of letters and digits.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

/// Whether a ` - ` part is a remaster or guest tag, not part of the name.
fn is_tag_part(part: &str) -> bool {
    words(part).any(|w| REMASTER.contains(&w) || FEAT.contains(&w))
}

/// Normalized words of `text`, in order.
#[must_use]
pub fn tokens(text: &str) -> Vec<String> {
    let folded: String = text.nfkc().collect::<String>().to_lowercase();
    let plain = without_brackets(&folded);
    let mut parts = plain.split(" - ");
    let mut kept = parts.next().unwrap_or_default().to_owned();
    for part in parts {
        if !is_tag_part(part) {
            kept.push(' ');
            kept.push_str(part);
        }
    }
    words(&kept)
        .take_while(|w| !FEAT.contains(w))
        .filter(|w| !REMASTER.contains(w))
        .map(str::to_owned)
        .collect()
}

/// Normalized text: [`tokens`] joined by single spaces.
#[must_use]
pub fn normalize(text: &str) -> String {
    tokens(text).join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guests_brackets_and_remaster_tags_are_removed() {
        for (raw, want) in [
            ("Song (feat. X)", "song"),
            ("Song [Remastered 2011]", "song"),
            ("Song - Remastered 2011", "song"),
            ("Song - 2011 Remaster", "song"),
            ("Song feat. Guest Star", "song"),
            ("Song ft. Guest", "song"),
            ("Artist featuring Guest", "artist"),
            ("Song - Live at Home", "song live at home"),
            ("Ｓｏｎｇ　Ｗｉｄｅ", "song wide"),
            ("Don't Stop Me Now", "don t stop me now"),
            ("Song (unclosed", "song unclosed"),
            ("Bài Hát Việt", "bài hát việt"),
        ] {
            assert_eq!(normalize(raw), want, "{raw}");
        }
    }

    #[test]
    fn empty_and_punctuation_only_text_gives_no_words() {
        assert!(tokens("").is_empty());
        assert!(tokens(" - ( ) ").is_empty());
    }
}
