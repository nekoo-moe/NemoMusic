//! The colour mesh behind the window (song-colour-theme spec): three or four soft colour fields
//! from the dominant colours of the cover, built on the cover worker thread. Pure maths, no GUI
//! dependency; the window draws each field as one pre-rendered radial texture.
//!
//! The fields come from hue buckets of the cover (centres at least [`MIN_HUE_GAP`] degrees
//! apart, weighted by chroma), so two neighbouring reds never take two of the four slots. A
//! grey cover gives greys; a cover of one hue gives that hue in two slightly turned variants.
//! Each colour is toned into a lightness band per base mode with richer chroma than the accent
//! palette (which keeps `palette::MAX_CHROMA`). The bands keep every text ink at 4.5:1 over the
//! panes, and over the bare field for the two main inks (tests, heartbeat peak included).

#![allow(
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // colour maths on small bounded values: short names are the standard notation

use crate::canvas::{MODES, mode_index};
use crate::drift;
use crate::palette::{GREY_CHROMA, Oklch, oklch_to_rgb, rgb_to_oklch};
use crate::scheme::BaseMode;
use crate::theme::Rgba;

#[cfg(test)]
mod tests;

/// Most colour fields.
pub const FIELDS: usize = 4;
/// Fewest colour fields (a cover with one or two hues is padded with turned variants).
pub const MIN_FIELDS: usize = 3;
/// Edge length of one field texture, px. The GPU scales it up, which only softens it more.
pub const BLOB_PX: u32 = 64;
/// Size of the still texture of the CPU renderer, px: 16:10, the default window shape.
pub const STILL_PX: (u32, u32) = (64, 40);
/// Opacity of a field at its centre.
pub const BLOB_PEAK: f32 = 0.9;
/// Fewest degrees between the hues of two fields taken from the cover.
pub const MIN_HUE_GAP: f64 = 40.0;
/// Degrees a padded variant turns from the first colour.
const PAD_TURN: f64 = 32.0;
/// A hue bucket needs this share of the coloured weight to take a slot (the first always does).
const MIN_SHARE: f64 = 0.06;
/// Fraction of coloured pixels below which the cover counts as grey.
const GREY_IMAGE_FRACTION: f64 = 0.02;
/// Chroma under which a pick is a grey.
const GREY_PICK: f64 = 0.02;
const HUE_BUCKETS: usize = 12;

/// One dominant colour of the cover, in OKLCH (lightness 0 to 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pick {
    /// Lightness.
    pub l: f64,
    /// Chroma, 0 for a grey.
    pub c: f64,
    /// Hue in degrees.
    pub h: f64,
}

/// The mesh of one cover for every base mode in [`MODES`] order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mesh {
    /// Number of fields in use ([`MIN_FIELDS`] to [`FIELDS`]).
    pub n: usize,
    /// The flat colour under the fields, per mode.
    pub base: [Rgba; 3],
    /// Toned colour of each field, per mode (entries from `n` on are unused).
    pub colors: [[Rgba; FIELDS]; 3],
    /// Field textures, per mode and field: [`BLOB_PX`] square RGBA8, straight alpha. Empty from
    /// field `n` on.
    pub textures: [[Vec<u8>; FIELDS]; 3],
    /// The fields composited over the flat colour at their resting poses, per mode: one
    /// [`STILL_PX`] RGBA8 image. The CPU renderer draws this instead of the fields (one
    /// full-window image instead of four large ones).
    pub stills: [Vec<u8>; 3],
}

impl Mesh {
    /// The flat colour under the fields for `base`.
    #[must_use]
    pub fn under(&self, base: BaseMode) -> Rgba {
        self.base[mode_index(base)]
    }

    /// The texture of field `i` for `base`.
    #[must_use]
    pub fn texture(&self, base: BaseMode, i: usize) -> &[u8] {
        &self.textures[mode_index(base)][i]
    }

    /// The still texture for `base`.
    #[must_use]
    pub fn still(&self, base: BaseMode) -> &[u8] {
        &self.stills[mode_index(base)]
    }

    /// The colours in use for `base`.
    #[must_use]
    pub fn used(&self, base: BaseMode) -> &[Rgba] {
        &self.colors[mode_index(base)][..self.n]
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Bucket {
    weight: f64,
    l: f64,
    a: f64,
    b: f64,
}

/// Circular distance between two hues, degrees.
fn hue_gap(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// The dominant colours of an RGBA8 image (a 64 px sample is plenty): between [`MIN_FIELDS`]
/// and [`FIELDS`] of them, strongest first.
#[must_use]
pub fn picks(rgba: &[u8]) -> Vec<Pick> {
    let mut buckets = [Bucket::default(); HUE_BUCKETS];
    let mut lights = Vec::new();
    let (mut total, mut coloured) = (0.0_f64, 0.0_f64);
    for p in rgba.as_chunks::<4>().0 {
        let o = rgb_to_oklch(Rgba {
            r: p[0],
            g: p[1],
            b: p[2],
            a: 255,
        });
        total += 1.0;
        lights.push(o.l);
        if o.c < GREY_CHROMA {
            continue;
        }
        coloured += 1.0;
        let i = ((o.h / 30.0) as usize).min(HUE_BUCKETS - 1);
        let rad = o.h.to_radians();
        let k = &mut buckets[i];
        k.weight += o.c;
        k.l += o.l * o.c;
        k.a += o.c * rad.cos() * o.c;
        k.b += o.c * rad.sin() * o.c;
    }
    let mut out = if total < 1.0 || coloured / total < GREY_IMAGE_FRACTION {
        grey_picks(&mut lights)
    } else {
        coloured_picks(&buckets)
    };
    pad(&mut out);
    out
}

/// Greys at the 20th, 50th and 80th percentile of the lightness.
fn grey_picks(lights: &mut [f64]) -> Vec<Pick> {
    lights.sort_by(f64::total_cmp);
    let last = (lights.len() as f64 - 1.0).max(0.0);
    [0.2, 0.5, 0.8]
        .map(|q| Pick {
            l: lights.get((last * q) as usize).copied().unwrap_or(0.5),
            c: 0.0,
            h: 0.0,
        })
        .to_vec()
}

/// The strongest hue buckets whose hues are at least [`MIN_HUE_GAP`] apart.
fn coloured_picks(buckets: &[Bucket; HUE_BUCKETS]) -> Vec<Pick> {
    let sum: f64 = buckets.iter().map(|k| k.weight).sum();
    let mut order: Vec<&Bucket> = buckets.iter().filter(|k| k.weight > 0.0).collect();
    order.sort_by(|x, y| y.weight.total_cmp(&x.weight));
    let mut out: Vec<Pick> = Vec::new();
    for k in order {
        if out.len() == FIELDS || (!out.is_empty() && k.weight / sum < MIN_SHARE) {
            continue;
        }
        let (ma, mb) = (k.a / k.weight, k.b / k.weight);
        let pick = Pick {
            l: k.l / k.weight,
            c: ma.hypot(mb),
            h: mb.atan2(ma).to_degrees().rem_euclid(360.0),
        };
        if out.iter().all(|q| hue_gap(q.h, pick.h) >= MIN_HUE_GAP) {
            out.push(pick);
        }
    }
    out
}

/// Fill up to [`MIN_FIELDS`] with variants of the first colour: turned a little each way and
/// lighter or darker.
fn pad(out: &mut Vec<Pick>) {
    let Some(first) = out.first().copied() else {
        return;
    };
    let mut k = 0_u32;
    while out.len() < MIN_FIELDS {
        let sign = if k.is_multiple_of(2) { 1.0 } else { -1.0 };
        out.push(Pick {
            l: (first.l - sign * 0.12).clamp(0.0, 1.0),
            c: first.c,
            h: if first.c < GREY_PICK {
                0.0
            } else {
                (first.h + sign * PAD_TURN).rem_euclid(360.0)
            },
        });
        k += 1;
    }
}

/// Lightness band, chroma limits and chroma boost of one base mode.
#[derive(Debug, Clone, Copy)]
struct Band {
    lo: f64,
    hi: f64,
    c_min: f64,
    c_max: f64,
    boost: f64,
    /// Lightness of the flat colour under the fields.
    under: f64,
}

const fn band(base: BaseMode) -> Band {
    match base {
        BaseMode::Dark => Band {
            lo: 0.21,
            hi: 0.30,
            c_min: 0.07,
            c_max: 0.16,
            boost: 1.8,
            under: 0.14,
        },
        BaseMode::Amoled => Band {
            lo: 0.12,
            hi: 0.27,
            c_min: 0.07,
            c_max: 0.16,
            boost: 1.8,
            under: 0.06,
        },
        BaseMode::Light => Band {
            lo: 0.86,
            hi: 0.94,
            c_min: 0.035,
            c_max: 0.09,
            boost: 1.2,
            under: 0.95,
        },
    }
}

/// One pick moved into the band of `base`: its own lightness picks the place in the band.
#[must_use]
pub fn toned(p: Pick, base: BaseMode) -> Rgba {
    let b = band(base);
    let t = ((p.l - 0.25) / 0.55).clamp(0.0, 1.0);
    let c = if p.c < GREY_PICK {
        0.0
    } else {
        (p.c * b.boost).clamp(b.c_min, b.c_max)
    };
    oklch_to_rgb(Oklch {
        l: b.lo + (b.hi - b.lo) * t,
        c,
        h: p.h,
    })
}

/// The flat colour under the fields: the first pick's hue, very quiet.
fn under(first: Pick, base: BaseMode) -> Rgba {
    oklch_to_rgb(Oklch {
        l: band(base).under,
        c: if first.c < GREY_PICK { 0.0 } else { 0.04 },
        h: first.h,
    })
}

/// One field texture: the colour at a smooth radial opacity, [`BLOB_PEAK`] at the centre and 0
/// at the edge.
fn blob(color: Rgba) -> Vec<u8> {
    let px = BLOB_PX as usize;
    let mut out = Vec::with_capacity(px * px * 4);
    for y in 0..px {
        for x in 0..px {
            let unit = |v: usize| (v as f64 + 0.5) / px as f64 * 2.0 - 1.0;
            let (dx, dy) = (unit(x), unit(y));
            let d2 = (dx * dx + dy * dy).min(1.0);
            let falloff = (1.0 - d2) * (1.0 - d2);
            let alpha = (falloff * f64::from(BLOB_PEAK) * 255.0).round() as u8;
            out.extend_from_slice(&[color.r, color.g, color.b, alpha]);
        }
    }
    out
}

/// The fields composited over `under` at their resting poses (`drift::pose` at 0), drawn the way
/// the window draws them: each over the one before, opacity [`BLOB_PEAK`] times the falloff.
fn still(colors: &[Rgba], under: Rgba) -> Vec<u8> {
    let (w, h) = (STILL_PX.0 as usize, STILL_PX.1 as usize);
    let aspect = f64::from(STILL_PX.1) / f64::from(STILL_PX.0);
    let poses: Vec<drift::Pose> = (0..colors.len())
        .map(|i| drift::pose(0.0, u8::try_from(i).unwrap_or(0)))
        .collect();
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            // Window space in units of its larger side (the width here).
            let (u, v) = (
                (x as f64 + 0.5) / w as f64,
                (y as f64 + 0.5) / h as f64 * aspect,
            );
            let mut px = [under.r, under.g, under.b].map(f64::from);
            for (i, c) in colors.iter().enumerate() {
                let (ax, ay, r) = drift::ANCHORS[i];
                let p = poses[i];
                let (cx, cy) = (
                    f64::from(ax + p.dx),
                    f64::from(ay) * aspect + f64::from(p.dy),
                );
                let rad = f64::from(r * p.scale);
                let d2 = ((u - cx).powi(2) + (v - cy).powi(2)) / (rad * rad);
                if d2 < 1.0 {
                    let a = f64::from(BLOB_PEAK) * (1.0 - d2) * (1.0 - d2);
                    for (k, ch) in [c.r, c.g, c.b].into_iter().enumerate() {
                        px[k] = px[k] * (1.0 - a) + f64::from(ch) * a;
                    }
                }
            }
            out.extend(px.map(|v| v.round() as u8));
            out.push(255);
        }
    }
    out
}

/// Build the mesh from an RGBA8 sample of the cover. Runs on a worker thread.
#[must_use]
pub fn build(rgba: &[u8]) -> Mesh {
    let found = picks(rgba);
    let first = found.first().copied().unwrap_or(Pick {
        l: 0.5,
        c: 0.0,
        h: 0.0,
    });
    let mut colors = [[Rgba::from_rgb_hex(0); FIELDS]; 3];
    let mut textures: [[Vec<u8>; FIELDS]; 3] = std::array::from_fn(|_| Default::default());
    for base in MODES {
        let m = mode_index(base);
        for (i, p) in found.iter().enumerate() {
            colors[m][i] = toned(*p, base);
            textures[m][i] = blob(colors[m][i]);
        }
    }
    let n = found.len().clamp(MIN_FIELDS, FIELDS);
    let base = MODES.map(|b| under(first, b));
    let stills = MODES.map(|b| {
        let m = mode_index(b);
        still(&colors[m][..n], base[m])
    });
    Mesh {
        n,
        base,
        colors,
        textures,
        stills,
    }
}
