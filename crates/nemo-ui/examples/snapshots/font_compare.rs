//! The lyric font comparison of lyrics-polish task 3: the same Vietnamese lines in the current
//! font and in each candidate, at the full-screen lyric sizes (active 32/40 at 600, neighbour
//! 24/32). The candidate files are not bundled: set `NEMO_FONT_DIR` to a folder with
//! `Inter-SemiBold.ttf`, `InterDisplay-SemiBold.ttf`, `BeVietnamPro-SemiBold.ttf` and
//! `PlusJakartaSans-SemiBold.ttf` (all SIL OFL 1.1) and render `lyrics-font-compare`.

use std::borrow::Cow;
use std::path::Path;

use iced::font::{Family, Weight};
use iced::widget::{column, container, text};
use iced::{Element, Font, Length, Pixels};
use nemo_ui::gui::app::Message;
use nemo_ui::gui::headless::render_view_png_with;

/// `(label, family name, file)`; the first is the bundled font.
const FACES: [(&str, &str, &str); 5] = [
    ("Google Sans Flex SemiBold (today)", "Google Sans Flex", ""),
    ("Inter SemiBold", "Inter", "Inter-SemiBold.ttf"),
    (
        "Inter Display SemiBold",
        "Inter Display",
        "InterDisplay-SemiBold.ttf",
    ),
    (
        "Be Vietnam Pro SemiBold",
        "Be Vietnam Pro",
        "BeVietnamPro-SemiBold.ttf",
    ),
    (
        "Plus Jakarta Sans SemiBold",
        "Plus Jakarta Sans",
        "PlusJakartaSans-SemiBold.ttf",
    ),
];

/// Lines with stacked marks (circumflex or breve plus a tone) in both cases.
const ACTIVE: &str = "Ng\u{1b0}\u{1edd}i \u{1ea5}y \u{111}\u{e3} r\u{1edd}i xa, \u{1ebf}m \u{1ea3}nh \u{1ec1} \u{1eab}n \u{1eaf}";
const CAPS: &str =
    "M\u{1edA}I QUEN NHAU \u{110}\u{c3} R\u{1ee6} \u{b7} \u{1ea8}N \u{1ec2}M \u{1ed6}I \u{1eb2}";
const NEIGHBOUR: &str = "v\u{e0} t\u{e2}m h\u{1ed3}n t\u{f4}i l\u{e0} m\u{1ed9}t b\u{1ea3}n nh\u{1ea1}c ch\u{1b0}a ai h\u{e1}t h\u{1ebf}t";

fn line<'a>(
    s: &'a str,
    family: &'static str,
    size: f32,
    line_h: f32,
    ink: iced::Color,
) -> Element<'a, Message> {
    text(s)
        .font(Font {
            family: Family::Name(family),
            weight: Weight::Semibold,
            ..Font::DEFAULT
        })
        .size(Pixels(size))
        .line_height(text::LineHeight::Absolute(Pixels(line_h)))
        .color(ink)
        .into()
}

fn view<'a>() -> Element<'a, Message> {
    let ink = iced::Color::from_rgb8(0xF2, 0xF3, 0xF5);
    let dim = iced::Color::from_rgb8(0xA6, 0xAB, 0xB5);
    let mut c = column![].spacing(20);
    for (label, family, _) in FACES {
        c = c.push(
            column![
                text(label).size(Pixels(13.0)).color(dim),
                line(ACTIVE, family, 32.0, 40.0, ink),
                line(CAPS, family, 32.0, 40.0, ink),
                line(NEIGHBOUR, family, 24.0, 32.0, dim),
            ]
            .spacing(2),
        );
    }
    container(c)
        .padding(32)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Color::from_rgb8(0x2A, 0x0A, 0x0E).into()),
            ..container::Style::default()
        })
        .into()
}

/// Render the comparison when it is named in `only` and `NEMO_FONT_DIR` is set.
pub(super) fn render(only: &[String], out_dir: &Path) -> Result<(), String> {
    const NAME: &str = "lyrics-font-compare";
    if !only.iter().any(|n| n == NAME) {
        return Ok(());
    }
    let dir =
        std::env::var_os("NEMO_FONT_DIR").ok_or("set NEMO_FONT_DIR for the font comparison")?;
    let mut extra: Vec<Cow<'static, [u8]>> = Vec::new();
    for (_, _, file) in FACES.iter().filter(|f| !f.2.is_empty()) {
        let bytes =
            std::fs::read(Path::new(&dir).join(file)).map_err(|e| format!("{file}: {e}"))?;
        extra.push(Cow::Owned(bytes));
    }
    let path = out_dir.join(format!("{NAME}.png"));
    render_view_png_with(view(), (1100, 1000), &path, extra)?;
    println!("wrote {}", path.display());
    Ok(())
}
