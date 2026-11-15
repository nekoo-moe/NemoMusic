//! Cover addresses sized for a slot (sharp-covers spec).
//!
//! The UI asks for a slot in device pixels (logical size times the window scale factor). That
//! size is rounded up to one of [`SIZE_STEPS`] so the disk cache keeps few sizes per cover. Then:
//! - `*.googleusercontent.com` and `*.ggpht.com` addresses carry a size suffix (`=w120-h120-l90-rj`,
//!   `=s120`); it is rewritten to the step, so a cover listed only at 60 and 120 px can still be
//!   fetched at 544 or 1080.
//! - `i.ytimg.com` video thumbnails come in fixed variants (`mqdefault`, `hqdefault`, `sddefault`,
//!   `hq720`, `maxresdefault`); the smallest one whose 1:1 crop covers the step is tried first,
//!   then larger ones, then smaller ones, because not every video has every variant.
//!
//! The listed address is always the last candidate, so nothing gets worse than before.

/// Device-pixel sizes cover art is fetched at.
pub const SIZE_STEPS: [u32; 4] = [120, 240, 544, 1080];

/// `i.ytimg.com` variants: file stem and the side of its 1:1 picture crop in px. `hqdefault` and
/// `sddefault` are 4:3 with the 16:9 picture letterboxed inside, so their crop is the picture
/// height. Ties keep this order (`maxresdefault` before `hq720`).
const YTIMG_VARIANTS: [(&str, u32); 5] = [
    ("mqdefault", 180),
    ("hqdefault", 270),
    ("sddefault", 360),
    ("maxresdefault", 720),
    ("hq720", 720),
];

/// File stems of `i.ytimg.com` thumbnails that may be swapped for another variant.
const YTIMG_STEMS: [&str; 6] = [
    "default",
    "mqdefault",
    "hqdefault",
    "sddefault",
    "maxresdefault",
    "hq720",
];

/// The smallest step that is at least `px`, else the largest step.
#[must_use]
pub fn size_step(px: u32) -> u32 {
    SIZE_STEPS
        .iter()
        .copied()
        .find(|&step| step >= px)
        .unwrap_or(SIZE_STEPS[SIZE_STEPS.len() - 1])
}

/// Addresses to try for a slot `px` device pixels wide, best first. The listed `url` is always
/// the last entry and appears once.
#[must_use]
pub fn candidates(url: &str, px: u32) -> Vec<String> {
    let step = size_step(px);
    let mut out = reqwest::Url::parse(url)
        .ok()
        .map(|parsed| match parsed.host_str() {
            Some(host) if is_google_image_host(host) => {
                resized_google(&parsed, step).into_iter().collect()
            }
            Some(host) if host.eq_ignore_ascii_case("i.ytimg.com") => ytimg_variants(&parsed, step),
            _ => Vec::new(),
        })
        .unwrap_or_default();
    out.retain(|candidate| candidate != url);
    out.push(url.to_owned());
    out
}

fn is_google_image_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    [".googleusercontent.com", ".ggpht.com"]
        .iter()
        .any(|suffix| {
            host.strip_suffix(suffix)
                .is_some_and(|label| !label.is_empty())
        })
}

/// The number after a one-letter option such as `w120`.
fn option_value(token: &str, letter: char) -> Option<u32> {
    let digits = token.strip_prefix(letter)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The size options of a suffix rewritten for `step`. `None` when the suffix names no size.
fn resized_options(options: &str, step: u32) -> Option<String> {
    let tokens: Vec<&str> = options.split('-').collect();
    let width = tokens.iter().find_map(|t| option_value(t, 'w'));
    let height = tokens.iter().find_map(|t| option_value(t, 'h'));
    let side = tokens.iter().any(|t| option_value(t, 's').is_some());
    if width.is_none() && height.is_none() && !side {
        return None;
    }
    // Keep the aspect of a `w`+`h` pair (banners are wider than tall).
    let new_height = match (width, height) {
        (Some(w), Some(h)) if w > 0 => {
            u32::try_from(u64::from(h) * u64::from(step) / u64::from(w)).unwrap_or(step)
        }
        _ => step,
    };
    let rewritten: Vec<String> = tokens
        .iter()
        .map(|t| {
            if option_value(t, 'w').is_some() {
                format!("w{step}")
            } else if option_value(t, 'h').is_some() {
                format!("h{}", new_height.max(1))
            } else if option_value(t, 's').is_some() {
                format!("s{step}")
            } else {
                (*t).to_owned()
            }
        })
        .collect();
    Some(rewritten.join("-"))
}

/// A `googleusercontent`/`ggpht` address with its size suffix set to `step`.
fn resized_google(url: &reqwest::Url, step: u32) -> Option<String> {
    let path = url.path();
    let last_slash = path.rfind('/')?;
    // The options start at the first `=` of the last path segment (`fcrop64=` may follow).
    let eq = last_slash + path[last_slash..].find('=')?;
    let options = resized_options(&path[eq + 1..], step)?;
    let mut out = url.clone();
    out.set_path(&format!("{}={options}", &path[..eq]));
    Some(out.to_string())
}

/// `i.ytimg.com/vi/<id>/<variant>.jpg` addresses for `step`, best first.
fn ytimg_variants(url: &reqwest::Url, step: u32) -> Vec<String> {
    let segments: Vec<&str> = url
        .path_segments()
        .map(Iterator::collect)
        .unwrap_or_default();
    let [folder, id, file] = segments.as_slice() else {
        return Vec::new();
    };
    let file_stem = file.split('.').next().unwrap_or_default();
    let id_ok = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !matches!(*folder, "vi" | "vi_webp") || !id_ok || !YTIMG_STEMS.contains(&file_stem) {
        return Vec::new();
    }
    let mut order: Vec<(usize, &(&str, u32))> = YTIMG_VARIANTS.iter().enumerate().collect();
    // Big enough ones from the smallest up, then the rest from the largest down.
    order.sort_by_key(|(rank, (_, side))| {
        if *side >= step {
            (0, *side, *rank)
        } else {
            (1, u32::MAX - side, *rank)
        }
    });
    order
        .into_iter()
        .map(|(_, (name, _))| format!("https://i.ytimg.com/vi/{id}/{name}.jpg"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{SIZE_STEPS, candidates, size_step};

    #[test]
    fn sizes_round_up_to_a_few_steps() {
        assert_eq!(size_step(0), 120);
        assert_eq!(size_step(120), 120);
        assert_eq!(size_step(121), 240);
        assert_eq!(size_step(320), 544);
        assert_eq!(size_step(960), 1080);
        assert_eq!(size_step(5000), 1080);
        assert!(SIZE_STEPS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn cover_url_asks_for_the_slot_size_times_scale() {
        let listed = "https://lh3.googleusercontent.com/abc_DEF-1=w120-h120-l90-rj";
        // A 160 px card at 200 % is 320 device px: the 544 step.
        assert_eq!(
            candidates(listed, 320),
            vec![
                "https://lh3.googleusercontent.com/abc_DEF-1=w544-h544-l90-rj".to_owned(),
                listed.to_owned(),
            ]
        );
        assert_eq!(
            candidates("https://yt3.ggpht.com/x/y=s88-c-k-c0x00ffffff-no-rj", 100)[0],
            "https://yt3.ggpht.com/x/y=s120-c-k-c0x00ffffff-no-rj"
        );
        assert_eq!(
            candidates("https://yt3.googleusercontent.com/z=s60", 960)[0],
            "https://yt3.googleusercontent.com/z=s1080"
        );
    }

    #[test]
    fn banner_suffix_keeps_its_aspect() {
        let listed = "https://lh3.googleusercontent.com/b=w1440-h600-p-l90-rj";
        assert_eq!(
            candidates(listed, 1000)[0],
            "https://lh3.googleusercontent.com/b=w1080-h450-p-l90-rj"
        );
        // Options after `fcrop64=` stay as they are; only the size changes.
        let cropped = "https://lh3.googleusercontent.com/c=w2120-fcrop64=1,00005a57ffffa5a8-k-nd";
        assert_eq!(
            candidates(cropped, 200)[0],
            "https://lh3.googleusercontent.com/c=w240-fcrop64=1,00005a57ffffa5a8-k-nd"
        );
    }

    #[test]
    fn addresses_without_a_size_are_used_as_listed() {
        for url in [
            "https://lh3.googleusercontent.com/no-suffix",
            "https://lh3.googleusercontent.com/d=rj",
            "https://music.youtube.com/img/cover.png",
            "https://googleusercontent.com/e=s120",
            "not a url",
        ] {
            assert_eq!(candidates(url, 500), vec![url.to_owned()], "{url}");
        }
    }

    #[test]
    fn video_thumbnail_tries_the_smallest_variant_that_covers_the_slot_first() {
        let listed = "https://i.ytimg.com/vi/dQw4w9WgXcQ/sddefault.jpg?sqp=-oay&rs=AOn4";
        let v = |name: &str| format!("https://i.ytimg.com/vi/dQw4w9WgXcQ/{name}.jpg");
        assert_eq!(
            candidates(listed, 200),
            vec![
                v("hqdefault"),
                v("sddefault"),
                v("maxresdefault"),
                v("hq720"),
                v("mqdefault"),
                listed.to_owned(),
            ]
        );
        assert_eq!(
            candidates(listed, 960),
            vec![
                v("maxresdefault"),
                v("hq720"),
                v("sddefault"),
                v("hqdefault"),
                v("mqdefault"),
                listed.to_owned(),
            ]
        );
        let webp = "https://i.ytimg.com/vi_webp/abc-_09/default.webp";
        assert_eq!(
            candidates(webp, 100)[0],
            "https://i.ytimg.com/vi/abc-_09/mqdefault.jpg"
        );
    }

    #[test]
    fn unknown_video_thumbnails_are_left_alone() {
        for url in [
            "https://i.ytimg.com/vi/abc/frame0.jpg",
            "https://i.ytimg.com/an_webp/abc/mqdefault_6s.webp",
            "https://i.ytimg.com/vi/a%2Fb/hqdefault.jpg",
            "https://i.ytimg.com/hqdefault.jpg",
        ] {
            assert_eq!(candidates(url, 500), vec![url.to_owned()], "{url}");
        }
    }
}
