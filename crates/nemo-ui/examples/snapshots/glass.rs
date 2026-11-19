//! Shots of the see-through window (appearance-glass). The window renders with a transparent
//! window colour and is composited over a mock wallpaper (a gradient, a few shapes and a mock
//! window of another app), so each PNG shows what is visible through the gaps and the edges.
//! Panes, the player bar and menus stay solid enough to read.

use std::path::Path;

use image::{Rgba, RgbaImage};
use nemo_ui::UiSettings;
use nemo_ui::gui::headless::glass::{physical, render_over};
use nemo_ui::gui::headless::{Scenario, build_app};
use nemo_ui::mock::mock_backend;
use nemo_ui::scheme::BaseMode;

/// The window sizes of the proof.
const SIZES: [(u32, u32); 2] = [(1280, 800), (960, 600)];
/// The window opacities of the proof, in percent.
const OPACITIES: [u8; 4] = [40, 60, 80, 95];

/// The mock wallpaper: a gradient, a few shapes and a mock window of another app.
struct Paper {
    width: f32,
    height: f32,
    light: bool,
}

impl Paper {
    /// The gradient colour at `t` (0 to 1, top left to bottom right).
    fn gradient(&self, t: f32) -> [f32; 3] {
        let stops: [[f32; 3]; 3] = if self.light {
            [
                [70.0, 150.0, 255.0],
                [255.0, 110.0, 160.0],
                [255.0, 190.0, 40.0],
            ]
        } else {
            [
                [24.0, 40.0, 110.0],
                [122.0, 40.0, 140.0],
                [232.0, 100.0, 60.0],
            ]
        };
        let (from, to, along) = if t < 0.5 {
            (stops[0], stops[1], t * 2.0)
        } else {
            (stops[1], stops[2], (t - 0.5) * 2.0)
        };
        [0, 1, 2].map(|i| from[i] + (to[i] - from[i]) * along)
    }

    /// The colour of the pixel at `(px, py)`. Pure arithmetic, so the proof never changes
    /// between runs.
    fn pixel(&self, px: u32, py: u32) -> [u8; 3] {
        // Pixel positions are small whole numbers.
        #[allow(clippy::cast_precision_loss)]
        let (x, y) = (px as f32, py as f32);
        let (width, height) = (self.width, self.height);
        let mut c = self.gradient((x / width * 0.6 + y / height * 0.4).clamp(0.0, 1.0));
        let disc = |cx: f32, cy: f32, r: f32| (x - cx).powi(2) + (y - cy).powi(2) < r * r;
        if disc(width * 0.12, height * 0.20, height * 0.14) {
            c = [255.0, 214.0, 64.0];
        } else if disc(width * 0.90, height * 0.80, height * 0.17) {
            c = [64.0, 220.0, 230.0];
        } else if (x + y * 0.5) % (width * 0.18) < width * 0.012 && y > height * 0.45 {
            c = [255.0, 255.0, 255.0];
        }
        // A mock window of another app, top right: title bar, text lines, white page.
        let (left, right, top, bottom) = (width * 0.55, width * 0.99, height * 0.02, height * 0.30);
        if x > left && x < right && y > top && y < bottom {
            let title = y < top + height * 0.04;
            let line = ((y - top) / (height * 0.03)).floor() % 2.0 < 1.0 && x > left + width * 0.02;
            c = if title {
                [40.0, 40.0, 46.0]
            } else if line {
                [200.0, 205.0, 215.0]
            } else {
                [250.0, 250.0, 252.0]
            };
        }
        // Colours are within 0 to 255.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let out = c.map(|v| v.clamp(0.0, 255.0) as u8);
        out
    }
}

/// The mock wallpaper at the physical size of a window of `size` logical px.
pub fn wallpaper(size: (u32, u32), light: bool) -> RgbaImage {
    let (pw, ph) = physical(size);
    // Picture sizes are small whole numbers.
    #[allow(clippy::cast_precision_loss)]
    let paper = Paper {
        width: pw as f32,
        height: ph as f32,
        light,
    };
    RgbaImage::from_fn(pw, ph, |px, py| {
        let [red, green, blue] = paper.pixel(px, py);
        Rgba([red, green, blue, 255])
    })
}

/// One shot: the window at `size` in `base`, with the glass at `opacity` percent.
pub fn scenario(size: (u32, u32), base: BaseMode, opacity: u8) -> Scenario {
    Scenario {
        size,
        track: 0,
        position_secs: 83,
        playing: true,
        settings: UiSettings {
            tos_acknowledged: true,
            base,
            window_glass: true,
            window_glass_opacity: opacity,
            ..UiSettings::default()
        },
        ..Scenario::default()
    }
}

/// Render the see-through shots named in `only` (all when it is empty).
pub fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    for size in SIZES {
        for (tag, base, light) in [
            ("dark", BaseMode::Dark, false),
            ("light", BaseMode::Light, true),
        ] {
            let paper = wallpaper(size, light);
            for opacity in OPACITIES {
                let name = format!("glass-{tag}-{opacity}-{}x{}", size.0, size.1);
                if !only.is_empty() && !only.contains(&name) {
                    continue;
                }
                let app = build_app(&mock_backend(), &scenario(size, base, opacity));
                let path = out_dir.join(format!("{name}.png"));
                render_over(&app, size, &paper, &path, true).map_err(|e| format!("{name}: {e}"))?;
                println!("wrote {}", path.display());
            }
        }
    }
    Ok(())
}
