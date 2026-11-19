//! Snapshots of the see-through window (appearance-glass spec). The app renders with a
//! transparent window colour, as it does when the window glass is clear; the picture is then
//! composited over a mock wallpaper, so the PNG shows what the desktop shows through the gaps.

use std::path::Path;

use iced::{Color, Pixels, Size};
use iced_test::Simulator;
use image::{Rgba, RgbaImage};

use super::{SNAPSHOT_SCALE, scratch_dir};
use crate::gui::app::App;
use crate::gui::{assets, style};

/// The iced theme of the app with a fully transparent window colour.
fn clear_theme() -> iced::Theme {
    let palette = iced::Theme::palette(&style::theme());
    iced::Theme::custom(
        "NemoMusic glass".to_owned(),
        iced::theme::Palette {
            background: Color::TRANSPARENT,
            ..palette
        },
    )
}

/// Render the app at `size` logical px with a transparent window colour. The result is the
/// 2x picture with premultiplied alpha, as the CPU renderer draws it.
///
/// # Errors
/// A message when the renderer cannot be created or the file cannot be read.
pub fn render_transparent(app: &App, size: (u32, u32)) -> Result<RgbaImage, String> {
    let settings = iced::Settings {
        fonts: assets::ALL_FONTS.iter().map(|f| (*f).into()).collect(),
        default_font: assets::font(false),
        default_text_size: Pixels(14.0),
        antialiasing: true,
        ..iced::Settings::default()
    };
    // Window sizes are small positive numbers.
    #[allow(clippy::cast_precision_loss)]
    let logical = Size::new(size.0 as f32, size.1 as f32);
    let mut sim = Simulator::with_size(settings, logical, app.view());
    let snapshot = sim.snapshot(&clear_theme()).map_err(|e| e.to_string())?;
    let dir = scratch_dir()?;
    snapshot
        .matches_image(dir.join("shot"))
        .map_err(|e| e.to_string())?;
    let produced = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "png"))
        .ok_or_else(|| "no PNG was produced".to_owned())?;
    let image = image::open(&produced)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(image)
}

/// Composite a premultiplied window picture over an opaque wallpaper of the same size.
#[must_use]
pub fn composite(window: &RgbaImage, wallpaper: &RgbaImage) -> RgbaImage {
    let mut out = wallpaper.clone();
    for (x, y, w) in window.enumerate_pixels() {
        let Some(d) = out.get_pixel_mut_checked(x, y) else {
            continue;
        };
        let keep = 255 - u16::from(w[3]);
        let mix = |src: u8, dst: u8| -> u8 {
            let v = u16::from(src) + (u16::from(dst) * keep + 127) / 255;
            u8::try_from(v.min(255)).unwrap_or(255)
        };
        *d = Rgba([mix(w[0], d[0]), mix(w[1], d[1]), mix(w[2], d[2]), 255]);
    }
    out
}

/// The physical size of a snapshot of `size` logical px.
#[must_use]
pub fn physical(size: (u32, u32)) -> (u32, u32) {
    // The scale is a small whole number.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let s = SNAPSHOT_SCALE as u32;
    (size.0 * s, size.1 * s)
}

/// Render the app over `wallpaper` (physical size) and write a PNG at `out`, halved to 1x when
/// `half` is set to keep the file small.
///
/// # Errors
/// A message when rendering or writing fails.
pub fn render_over(
    app: &App,
    size: (u32, u32),
    wallpaper: &RgbaImage,
    out: &Path,
    half: bool,
) -> Result<(), String> {
    let window = render_transparent(app, size)?;
    let mut full = composite(&window, wallpaper);
    if half {
        full = image::imageops::resize(
            &full,
            full.width() / 2,
            full.height() / 2,
            image::imageops::FilterType::Triangle,
        );
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    full.save(out).map_err(|e| e.to_string())
}
