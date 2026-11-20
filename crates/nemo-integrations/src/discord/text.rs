//! Text Discord accepts: 2 to 128 bytes per line.

/// Fewest bytes Discord accepts in a line.
pub const MIN_BYTES: usize = 2;
/// Most bytes Discord accepts in a line.
pub const MAX_BYTES: usize = 128;

const ELLIPSIS: &str = "\u{2026}";

/// Make `text` fit a Discord line.
///
/// Control characters (new lines, tabs) become spaces and the ends are trimmed. Text longer than
/// [`MAX_BYTES`] is cut on a character boundary and ends in `…`. Text shorter than [`MIN_BYTES`]
/// is padded with spaces.
#[must_use]
pub fn fit(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let mut out = cleaned.trim().to_owned();
    if out.len() > MAX_BYTES {
        let mut end = MAX_BYTES - ELLIPSIS.len();
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        let kept = out.trim_end().len();
        out.truncate(kept);
        out.push_str(ELLIPSIS);
    }
    while out.len() < MIN_BYTES {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{MAX_BYTES, MIN_BYTES, fit};

    #[test]
    fn short_text_is_padded_to_two_bytes() {
        assert_eq!(fit(""), "  ");
        assert_eq!(fit("a"), "a ");
        assert_eq!(fit("ab"), "ab");
        // One 2-byte character already reaches the minimum.
        assert_eq!(fit("\u{3a9}"), "\u{3a9}");
    }

    #[test]
    fn long_text_is_cut_on_a_character_boundary() {
        // Each `é` is 2 bytes, so the cut point of byte 125 falls inside a character.
        let text = "\u{e9}".repeat(100);
        let out = fit(&text);
        assert!(out.len() <= MAX_BYTES, "{}", out.len());
        assert!(out.ends_with('\u{2026}'));
        assert!(
            out.trim_end_matches('\u{2026}')
                .chars()
                .all(|c| c == '\u{e9}')
        );
    }

    #[test]
    fn three_and_four_byte_characters_are_never_split() {
        for ch in ["\u{4e2d}", "\u{1f3b5}"] {
            let out = fit(&ch.repeat(200));
            assert!(out.len() <= MAX_BYTES);
            assert!(out.len() >= MIN_BYTES);
            assert!(out.ends_with('\u{2026}'));
        }
    }

    #[test]
    fn text_of_exactly_the_limit_is_kept() {
        let text = "a".repeat(MAX_BYTES);
        assert_eq!(fit(&text), text);
        let over = "a".repeat(MAX_BYTES + 1);
        let cut = fit(&over);
        assert_eq!(cut.len(), MAX_BYTES);
        assert!(cut.ends_with('\u{2026}'));
    }

    #[test]
    fn control_characters_become_spaces_and_ends_are_trimmed() {
        assert_eq!(fit("  a\nb\tc  "), "a b c");
        assert_eq!(fit("\n\n"), "  ");
    }
}
