//! The song-coloured canvas behind the right Now Playing pane and the full-screen Now Playing
//! (wave-lyrics criterion 12). Built on the cover worker thread from the pre-blurred backdrop:
//! every pixel keeps its hue, gets a little more chroma and has its lightness squeezed into a
//! band per base mode, so the canvas is clearly the cover's colour while every text ink keeps
//! 4.5:1 on any pixel of it (tests below). A grey cover has no chroma to boost and stays grey.
//! Static: no loop, no live blur (tier Low).

use crate::palette::{Oklch, mix_rgb, oklch_to_rgb, rgb_to_oklch};
use crate::scheme::BaseMode;
use crate::theme::{Neutrals, Rgba};

/// Gradient stops of the pane canvas (top to bottom bands of the cover).
pub const STOPS: usize = 5;
/// Base modes in [`Canvas`] order.
pub const MODES: [BaseMode; 3] = [BaseMode::Dark, BaseMode::Amoled, BaseMode::Light];

/// Opacity of `on-surface` for lyric pieces still to come and for the lines around the active
/// one, on dark surfaces (wave-lyrics criterion 8).
pub const COMING_ALPHA_DARK: f32 = 0.55;
/// The same on light surfaces: dark ink at 0.55 does not reach 4.5:1 there, 0.72 does.
pub const COMING_ALPHA_LIGHT: f32 = 0.72;
/// Opacity of `on-surface` for pieces already sung.
pub const SUNG_ALPHA: f32 = 0.9;

/// Lightness band, chroma cap and chroma boost of one base mode.
#[derive(Debug, Clone, Copy)]
struct Band {
    lo: f64,
    hi: f64,
    chroma_max: f64,
    boost: f64,
}

const fn band(base: BaseMode) -> Band {
    match base {
        BaseMode::Dark => Band {
            lo: 0.17,
            hi: 0.29,
            chroma_max: 0.10,
            boost: 1.7,
        },
        BaseMode::Amoled => Band {
            lo: 0.10,
            hi: 0.25,
            chroma_max: 0.10,
            boost: 1.7,
        },
        BaseMode::Light => Band {
            lo: 0.90,
            hi: 0.96,
            chroma_max: 0.045,
            boost: 1.2,
        },
    }
}

/// Index of `base` in [`MODES`] (and in the arrays of [`Canvas`]).
#[must_use]
pub const fn mode_index(base: BaseMode) -> usize {
    match base {
        BaseMode::Dark => 0,
        BaseMode::Amoled => 1,
        BaseMode::Light => 2,
    }
}

/// Opacity of the "coming" lyric ink for a base mode.
#[must_use]
pub const fn coming_alpha(base: BaseMode) -> f32 {
    if base.is_light() {
        COMING_ALPHA_LIGHT
    } else {
        COMING_ALPHA_DARK
    }
}

/// One cover pixel moved into the canvas band of `base`.
#[must_use]
pub fn retone(c: Rgba, base: BaseMode) -> Rgba {
    let b = band(base);
    let o = rgb_to_oklch(c);
    oklch_to_rgb(Oklch {
        l: b.lo + (b.hi - b.lo) * o.l.clamp(0.0, 1.0),
        c: (o.c * b.boost).min(b.chroma_max),
        h: o.h,
    })
}

/// The canvas of one cover for every base mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canvas {
    /// Edge length of the square textures, px.
    pub px: u32,
    /// Raw RGBA textures (straight alpha, opaque), one per base mode in [`MODES`] order.
    pub textures: [Vec<u8>; 3],
    /// Top-to-bottom gradient stops for the pane, one set per base mode.
    pub stops: [[Rgba; STOPS]; 3],
}

impl Canvas {
    /// The texture for `base`.
    #[must_use]
    pub fn texture(&self, base: BaseMode) -> &[u8] {
        &self.textures[mode_index(base)]
    }

    /// The gradient stops for `base`.
    #[must_use]
    pub fn stops(&self, base: BaseMode) -> [Rgba; STOPS] {
        self.stops[mode_index(base)]
    }
}

/// Build the canvas from a square, pre-blurred RGBA8 image `px` wide. Runs on a worker thread.
#[must_use]
pub fn build(rgba: &[u8], px: u32) -> Canvas {
    let pixels: Vec<Rgba> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Rgba {
            r: p[0],
            g: p[1],
            b: p[2],
            a: 255,
        })
        .collect();
    let bands = band_means(&pixels, px);
    let textures = MODES.map(|base| {
        let mut out = Vec::with_capacity(pixels.len() * 4);
        for p in &pixels {
            let t = retone(*p, base);
            out.extend_from_slice(&[t.r, t.g, t.b, 255]);
        }
        out
    });
    let stops = MODES.map(|base| bands.map(|c| retone(c, base)));
    Canvas {
        px,
        textures,
        stops,
    }
}

/// Mean colour of [`STOPS`] horizontal bands, top to bottom.
fn band_means(pixels: &[Rgba], px: u32) -> [Rgba; STOPS] {
    let w = usize::try_from(px.max(1)).unwrap_or(1);
    let rows = pixels.len() / w;
    let mut out = [Rgba::from_rgb_hex(0x80_8080); STOPS];
    for (k, slot) in out.iter_mut().enumerate() {
        let (y0, y1) = (
            k * rows / STOPS,
            ((k + 1) * rows / STOPS).max(k * rows / STOPS + 1),
        );
        let mut sum = [0_u64; 3];
        let mut n = 0_u64;
        for p in pixels.iter().skip(y0 * w).take((y1 - y0) * w) {
            sum[0] += u64::from(p.r);
            sum[1] += u64::from(p.g);
            sum[2] += u64::from(p.b);
            n += 1;
        }
        if n > 0 {
            // A mean of u8 values fits in u8.
            #[allow(clippy::cast_possible_truncation)]
            let ch = |i: usize| (sum[i] / n) as u8;
            *slot = Rgba {
                r: ch(0),
                g: ch(1),
                b: ch(2),
                a: 255,
            };
        }
    }
    out
}

/// Cross-fade two sets of stops (`t` 0 shows `from`, 1 shows `to`).
#[must_use]
pub fn mix_stops(from: [Rgba; STOPS], to: [Rgba; STOPS], t: f32) -> [Rgba; STOPS] {
    let t = f64::from(t.clamp(0.0, 1.0));
    let mut out = to;
    for (i, o) in out.iter_mut().enumerate() {
        *o = mix_rgb(from[i], to[i], t);
    }
    out
}

/// Every text ink drawn straight on the canvas of `base`: name and colour (with alpha). The
/// lines around the active lyric line use the `coming` ink.
#[must_use]
pub fn text_inks(n: &Neutrals, base: BaseMode) -> [(&'static str, Rgba); 4] {
    [
        ("on-surface", n.on_surface),
        ("on-surface-variant", n.on_surface_variant),
        ("sung", n.on_surface.with_alpha(SUNG_ALPHA)),
        ("coming", n.on_surface.with_alpha(coming_alpha(base))),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheme::{AccentIntensity, ThemeChoice, accent_palette};
    use crate::theme::contrast_ratio;

    /// The fixture covers of `design.md` section 2: all-black, all-white, grey, saturated, noisy.
    fn covers() -> Vec<(&'static str, Vec<u8>)> {
        let px = 24_u32;
        let solid = |c: [u8; 3]| -> Vec<u8> {
            (0..px * px).flat_map(|_| [c[0], c[1], c[2], 255]).collect()
        };
        let noisy: Vec<u8> = (0..px * px)
            .flat_map(|i| {
                let v = i.wrapping_mul(2_654_435_761);
                // Low bytes of a hash: pseudo-random channels.
                #[allow(clippy::cast_possible_truncation)]
                [v as u8, (v >> 8) as u8, (v >> 16) as u8, 255]
            })
            .collect();
        vec![
            ("black", solid([0, 0, 0])),
            ("white", solid([255, 255, 255])),
            ("grey", solid([128, 128, 128])),
            ("saturated", solid([255, 20, 140])),
            ("yellow", solid([250, 230, 0])),
            ("noisy", noisy),
        ]
    }

    #[test]
    fn every_ink_reaches_4_5_on_every_canvas_pixel_and_stop() {
        for (name, rgba) in covers() {
            let c = build(&rgba, 24);
            for base in MODES {
                let n = base.neutrals();
                let mut under: Vec<Rgba> = c
                    .texture(base)
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| Rgba {
                        r: p[0],
                        g: p[1],
                        b: p[2],
                        a: 255,
                    })
                    .collect();
                under.extend(c.stops(base));
                for bg in under {
                    for (ink, fg) in text_inks(&n, base) {
                        let ratio = contrast_ratio(fg.over(bg), bg);
                        assert!(ratio >= 4.5, "{name} {base:?} {ink}: {ratio:.2}");
                    }
                }
            }
        }
    }

    #[test]
    fn accent_text_stays_legible_on_the_canvas() {
        for (name, rgba) in covers() {
            let c = build(&rgba, 24);
            for base in MODES {
                for theme in ThemeChoice::ALL {
                    let seed = Some(crate::palette::Seed {
                        hue: 330.0,
                        chroma: 0.14,
                    });
                    let p = accent_palette(theme, base, AccentIntensity::Vivid, seed);
                    for bg in c.stops(base) {
                        let ratio = contrast_ratio(p.primary, bg);
                        assert!(ratio >= 4.5, "{name} {base:?} {theme:?}: {ratio:.2}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_grey_cover_stays_grey_and_a_coloured_one_keeps_its_hue() {
        let grey = build(&covers()[2].1, 24);
        for base in MODES {
            for s in grey.stops(base) {
                let spread = s.r.max(s.g).max(s.b) - s.r.min(s.g).min(s.b);
                assert!(spread <= 2, "{base:?}: {s:?} is not neutral");
            }
        }
        let pink = build(&covers()[3].1, 24);
        let o = rgb_to_oklch(pink.stops(BaseMode::Amoled)[0]);
        assert!(o.c > 0.05, "AMOLED keeps visible colour: {o:?}");
        assert!((o.h - rgb_to_oklch(Rgba::from_rgb_hex(0xFF_148C)).h).abs() < 12.0);
    }

    #[test]
    fn bands_follow_the_cover_top_to_bottom() {
        let px = 20_u32;
        let rgba: Vec<u8> = (0..px * px)
            .flat_map(|i| {
                if i / px < px / 2 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                }
            })
            .collect();
        let c = build(&rgba, px);
        let s = c.stops(BaseMode::Dark);
        assert!(s[0].r > s[0].b && s[STOPS - 1].b > s[STOPS - 1].r, "{s:?}");
        let m = mix_stops(s, c.stops(BaseMode::Light), 1.0);
        assert_eq!(m, c.stops(BaseMode::Light));
    }
}
