//! Real text measurement for ellipsis and line breaks.
//!
//! iced 0.14 text has no ellipsis, and a view cannot ask the renderer for a width. The bundled
//! font is known, so this reads its `head`, `hhea`, `hmtx` and `cmap` tables once and sums the
//! true glyph advances. Kerning is ignored (a fraction of a pixel per line) and the widest
//! bundled weight (`SemiBold`) is used, so a measured line never ends up wider than it was
//! counted. Characters the font lacks (CJK, emoji) count as one em, the width of a full-width
//! glyph in the system fallback fonts.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::assets::FONT_SEMIBOLD;

/// Glyph advances of the bundled font.
struct Metrics {
    units_per_em: f32,
    advance: HashMap<char, u16>,
    /// Advance used for characters the font does not have.
    fallback: f32,
}

fn be16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

fn be32(d: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_be_bytes([
        *d.get(at)?,
        *d.get(at + 1)?,
        *d.get(at + 2)?,
        *d.get(at + 3)?,
    ]) as usize)
}

/// Offset of the table `tag` in a TrueType file.
fn table(d: &[u8], tag: [u8; 4]) -> Option<usize> {
    let count = usize::from(be16(d, 4)?);
    (0..count).find_map(|i| {
        let rec = 12 + 16 * i;
        (d.get(rec..rec + 4)? == tag.as_slice()).then(|| be32(d, rec + 8))?
    })
}

/// Read the metrics of a TrueType font; `None` when a table is missing or malformed.
fn parse(d: &[u8]) -> Option<Metrics> {
    let head = table(d, *b"head")?;
    let units_per_em = f32::from(be16(d, head + 18)?);
    let hhea = table(d, *b"hhea")?;
    let long_metrics = usize::from(be16(d, hhea + 34)?).max(1);
    let hmtx = table(d, *b"hmtx")?;
    let advance_of = |glyph: u16| -> Option<u16> {
        let index = usize::from(glyph).min(long_metrics - 1);
        be16(d, hmtx + 4 * index)
    };

    let cmap = table(d, *b"cmap")?;
    let subtables = usize::from(be16(d, cmap + 2)?);
    let fmt4 = (0..subtables).find_map(|i| {
        let off = cmap + be32(d, cmap + 4 + 8 * i + 4)?;
        (be16(d, off)? == 4).then_some(off)
    })?;
    let segs = usize::from(be16(d, fmt4 + 6)?) / 2;
    let ends = fmt4 + 14;
    let starts = ends + 2 * segs + 2;
    let deltas = starts + 2 * segs;
    let offsets = deltas + 2 * segs;

    let mut advance = HashMap::new();
    for i in 0..segs {
        let (end, start) = (be16(d, ends + 2 * i)?, be16(d, starts + 2 * i)?);
        if start == 0xFFFF {
            continue;
        }
        let delta = be16(d, deltas + 2 * i)?;
        let range = be16(d, offsets + 2 * i)?;
        for c in start..=end {
            let glyph = if range == 0 {
                c.wrapping_add(delta)
            } else {
                let at = offsets + 2 * i + usize::from(range) + 2 * usize::from(c - start);
                match be16(d, at) {
                    Some(0) | None => 0,
                    Some(g) => g.wrapping_add(delta),
                }
            };
            if glyph != 0
                && let (Some(ch), Some(adv)) = (char::from_u32(u32::from(c)), advance_of(glyph))
            {
                advance.insert(ch, adv);
            }
        }
    }
    if advance.is_empty() {
        return None;
    }
    Some(Metrics {
        units_per_em,
        advance,
        fallback: units_per_em,
    })
}

fn metrics() -> Option<&'static Metrics> {
    static METRICS: OnceLock<Option<Metrics>> = OnceLock::new();
    METRICS.get_or_init(|| parse(FONT_SEMIBOLD)).as_ref()
}

/// Width in px of `s` at `size` px. Without readable metrics it falls back to 0.55 em a
/// character, the old estimate.
#[must_use]
pub fn width(s: &str, size: f32) -> f32 {
    match metrics() {
        Some(m) => {
            let units: f32 = s
                .chars()
                .map(|c| {
                    if c == '\n' {
                        0.0
                    } else {
                        m.advance.get(&c).map_or(m.fallback, |a| f32::from(*a))
                    }
                })
                .sum();
            units / m.units_per_em * size
        }
        // Character counts are small, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        None => s.chars().count() as f32 * size * 0.55,
    }
}

const ELLIPSIS: char = '\u{2026}';

/// Longest prefix of `s` (in chars) whose width plus an ellipsis fits `max_w`.
fn prefix_with_ellipsis(s: &str, max_w: f32, size: f32) -> usize {
    let room = max_w - width("\u{2026}", size);
    let mut w = 0.0;
    let mut n = 0;
    for (i, c) in s.chars().enumerate() {
        w += width(c.encode_utf8(&mut [0; 4]), size);
        if w > room {
            break;
        }
        n = i + 1;
    }
    n
}

/// Cut `s` to `keep` chars, preferring the end of a word when one lies in the last stretch.
fn cut_at_word(s: &str, keep: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let keep = keep.min(chars.len());
    // A cut inside a word moves back to the last space, if that loses less than 45 percent.
    let mid_word = chars.get(keep).is_some_and(|c| !c.is_whitespace())
        && keep > 0
        && !chars[keep - 1].is_whitespace();
    let mut end = keep;
    if mid_word && let Some(space) = chars[..keep].iter().rposition(|c| c.is_whitespace()) {
        // Char counts are small, exact in f32.
        #[allow(clippy::cast_precision_loss)]
        let kept_enough = space as f32 >= keep as f32 * 0.55;
        if kept_enough {
            end = space;
        }
    }
    let head: String = chars[..end].iter().collect();
    head.trim_end_matches(|c: char| c.is_whitespace() || ",;:-\u{2013}\u{2014}&(".contains(c))
        .to_owned()
}

/// Shorten `s` with an ellipsis so it fits `max_w` px at `size` px, cutting at a word end when
/// that keeps most of the text. Text that fits is returned unchanged.
#[must_use]
pub fn fit(s: &str, max_w: f32, size: f32) -> String {
    if width(s, size) <= max_w {
        return s.to_owned();
    }
    let keep = prefix_with_ellipsis(s, max_w, size).max(1);
    // One char that cannot fit is shown whole; an ellipsis alone would say less.
    if keep >= s.chars().count() {
        return s.to_owned();
    }
    let mut out = cut_at_word(s, keep);
    if out.is_empty() {
        out = s.chars().take(keep).collect();
    }
    out.push(ELLIPSIS);
    out
}

/// Break `s` into at most `lines` lines of at most `max_w` px each, at spaces where possible; the
/// last line ends with an ellipsis when text is left over. Lines are joined with `\n`.
#[must_use]
pub fn fit_lines(s: &str, max_w: f32, size: f32, lines: usize) -> String {
    let lines = lines.max(1);
    if lines == 1 {
        return fit(s, max_w, size);
    }
    let mut out: Vec<String> = Vec::new();
    let mut rest = s.trim().to_owned();
    while !rest.is_empty() {
        if out.len() + 1 == lines || width(&rest, size) <= max_w {
            out.push(fit(&rest, max_w, size));
            break;
        }
        // Largest prefix that fits, then back to a space when there is one.
        let mut w = 0.0;
        let mut n = 0;
        for (i, c) in rest.chars().enumerate() {
            w += width(c.encode_utf8(&mut [0; 4]), size);
            if w > max_w {
                break;
            }
            n = i + 1;
        }
        let n = n.max(1);
        let chars: Vec<char> = rest.chars().collect();
        let split = chars[..n]
            .iter()
            .rposition(|c| c.is_whitespace())
            .filter(|p| *p > 0)
            .unwrap_or(n);
        let head: String = chars[..split].iter().collect();
        out.push(head.trim_end().to_owned());
        chars[split..]
            .iter()
            .collect::<String>()
            .trim_start()
            .clone_into(&mut rest);
    }
    out.join("\n")
}

/// Number of rows `s` takes when wrapped to `max_w` px at `size` px the way iced's
/// `Wrapping::WordOrGlyph` does: at spaces, and inside a word that is wider than a row. Forced
/// line breaks (`\n`) start a new row. Always at least 1. The measure is the widest weight, so
/// the count is never lower than what is drawn.
#[must_use]
pub fn rows(s: &str, max_w: f32, size: f32) -> usize {
    let max_w = max_w.max(1.0);
    let space = width(" ", size);
    s.split('\n')
        .map(|para| {
            let mut rows = 1;
            let mut used = 0.0_f32;
            for word in para.split_whitespace() {
                let w = width(word, size);
                let need = if used > 0.0 { used + space + w } else { w };
                if need <= max_w {
                    used = need;
                    continue;
                }
                if used > 0.0 {
                    rows += 1;
                }
                used = 0.0;
                if w <= max_w {
                    used = w;
                    continue;
                }
                // A word wider than a row breaks between glyphs.
                for c in word.chars() {
                    let cw = width(c.encode_utf8(&mut [0; 4]), size);
                    if used + cw > max_w && used > 0.0 {
                        rows += 1;
                        used = 0.0;
                    }
                    used += cw;
                }
            }
            rows
        })
        .sum::<usize>()
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_wrap_at_spaces_and_break_long_words() {
        assert_eq!(rows("", 100.0, 16.0), 1);
        assert_eq!(rows("short", 200.0, 16.0), 1);
        let line = "và tâm hồn tôi là một bản nhạc chưa bao giờ được hát hết";
        let one = width(line, 20.0);
        let n = rows(line, one / 2.0 + 1.0, 20.0);
        assert!((2..=3).contains(&n), "{n}");
        assert_eq!(rows(line, one + 1.0, 20.0), 1);
        // A word wider than the row breaks between glyphs.
        let long = "Supercalifragilisticexpialidocious";
        assert!(rows(long, width(long, 14.0) / 3.0, 14.0) >= 3);
        assert_eq!(rows("a\nb", 100.0, 14.0), 2);
    }

    #[test]
    fn measured_width_is_real_and_scales_with_size() {
        let w14 = width("Hello world", 14.0);
        // Between 0.40 and 0.70 em per character for a humanist sans.
        assert!(
            w14 > 11.0 * 14.0 * 0.40 && w14 < 11.0 * 14.0 * 0.70,
            "{w14}"
        );
        assert!((width("Hello world", 28.0) - 2.0 * w14).abs() < 0.01);
        assert!(width("iiii", 14.0) < width("MMMM", 14.0));
    }

    #[test]
    fn vietnamese_letters_have_metrics() {
        let w = width("M\u{1edb}i Quen Nhau \u{110}\u{e3} R\u{1ee7}", 16.0);
        assert!(w > 60.0 && w < 200.0, "{w}");
    }

    #[test]
    fn text_that_fits_is_unchanged_and_long_text_gets_one_ellipsis() {
        assert_eq!(fit("short", 200.0, 14.0), "short");
        let s = fit("A very long title that cannot fit in here", 120.0, 14.0);
        assert!(s.ends_with(ELLIPSIS), "{s}");
        assert!(width(&s, 14.0) <= 120.0, "{s}: {}", width(&s, 14.0));
        assert_eq!(s.matches(ELLIPSIS).count(), 1);
    }

    #[test]
    fn cuts_land_on_word_ends_when_possible() {
        let s = fit("Playlist for a very quiet Sunday afternoon", 190.0, 14.0);
        let body = s.trim_end_matches(ELLIPSIS);
        assert!(
            "Playlist for a very quiet Sunday afternoon"
                .split(' ')
                .any(|w| body.ends_with(w)),
            "cut inside a word: {s}"
        );
    }

    #[test]
    fn a_single_long_word_is_cut_by_characters() {
        let s = fit("Supercalifragilisticexpialidocious", 100.0, 14.0);
        assert!(s.ends_with(ELLIPSIS) && width(&s, 14.0) <= 100.0, "{s}");
    }

    #[test]
    fn two_lines_break_at_spaces_and_end_with_an_ellipsis() {
        let text = "Midnight Afterglow with a much longer name than fits";
        let two = fit_lines(text, 120.0, 16.0, 2);
        let parts: Vec<&str> = two.split('\n').collect();
        assert_eq!(parts.len(), 2, "{two}");
        assert!(parts.iter().all(|p| width(p, 16.0) <= 120.0), "{two}");
        assert!(parts[0].ends_with(|c: char| c.is_alphanumeric()), "{two}");
        assert_eq!(fit_lines("Short one", 120.0, 16.0, 2), "Short one");
    }
}
