//! The ready-made skeleton shapes. Each has the outer size of the real shape it stands for
//! (a card is `size` square, a header is its type line high, ...), so nothing moves when the
//! content arrives.

use iced::widget::{Space, column, container, row, stack};
use iced::{Alignment, Element, Length, Padding};

use super::Skeleton;
use crate::gui::app::{ArtShape, Message};
use crate::gui::style::gap;
use crate::gui::views::kit::{chip_width, vspace};
use crate::layout::{CARD_MIN, Grid};
use crate::theme::{shape, type_scale};

/// Height of a chip.
pub const CHIP_H: f32 = 32.0;
/// Art of the Home hero banner, and its padding.
const HERO_ART: f32 = 176.0;
/// Height of the Home hero banner.
pub const HERO_H: f32 = HERO_ART + 2.0 * gap::XL;
/// Cover of the Top result card (`art.card`).
// 160 is exact in f32.
#[allow(clippy::cast_precision_loss)]
const TOP_ART: f32 = CARD_MIN as f32;
impl Skeleton {
    /// A card `size` px square, like a Home or search card: the cover with two text lines near
    /// its bottom edge.
    #[must_use]
    pub fn card<'a>(self, size: f32) -> Element<'a, Message> {
        let inner = size - 2.0 * gap::MD;
        let on = self.inset();
        let text = column![
            on.line(inner * 0.7, type_scale::TITLE_SM),
            on.line(inner * 0.45, type_scale::BODY_SM),
        ];
        stack![
            self.block(Length::Fixed(size), size, ArtShape::Tile.radius(size)),
            container(text)
                .width(Length::Fixed(size))
                .height(Length::Fixed(size))
                .align_y(Alignment::End)
                .padding(gap::MD),
        ]
        .into()
    }

    /// An artist card: a round cover with two centred lines under it.
    #[must_use]
    pub fn artist_card<'a>(self, size: f32) -> Element<'a, Message> {
        column![
            self.circle(size),
            vspace(gap::MD),
            self.line(size * 0.6, type_scale::BODY_MD),
            self.line(size * 0.4, type_scale::BODY_SM),
        ]
        .width(Length::Fixed(size))
        .align_x(Alignment::Center)
        .into()
    }

    /// `rows` rows of cards laid out like [`crate::gui::views::kit::card_grid`].
    #[must_use]
    pub fn cards<'a>(self, grid: Grid, rows: usize, round: bool) -> Element<'a, Message> {
        // Grid numbers are small exact numbers.
        #[allow(clippy::cast_precision_loss)]
        let (size, gap_px) = (grid.card as f32, grid.gap as f32);
        let mut all = column![].spacing(gap::XL);
        for _ in 0..rows {
            let mut line = row![].spacing(gap_px);
            for _ in 0..grid.columns {
                line = line.push(if round {
                    self.artist_card(size)
                } else {
                    self.card(size)
                });
            }
            all = all.push(line);
        }
        all.into()
    }

    /// A section title of `width` px (`title-lg`) with the gap under it, like `shelf_header`.
    #[must_use]
    pub fn section_header<'a>(self, width: f32) -> Element<'a, Message> {
        column![self.line(width, type_scale::TITLE_LG), vspace(gap::MD)].into()
    }

    /// A row of chips with these labels (their real widths), [`CHIP_H`] high.
    #[must_use]
    pub fn chip_row<'a>(self, labels: &[&str]) -> Element<'a, Message> {
        let mut r = row![].spacing(gap::SM);
        for text in labels {
            r = r.push(self.block(Length::Fixed(chip_width(text)), CHIP_H, shape::FULL));
        }
        r.into()
    }

    /// The Home daily-mix banner, `width` wide and [`HERO_H`] high: tag, title, subtitle, a Play
    /// pill and the cover.
    #[must_use]
    pub fn hero<'a>(self, width: f32) -> Element<'a, Message> {
        let text_w = (width - HERO_ART - 3.0 * gap::XL).max(200.0);
        let on = self.inset();
        let info = column![
            on.line(text_w * 0.2, type_scale::LABEL_SM),
            vspace(gap::XS),
            on.line(text_w * 0.55, type_scale::HEADLINE_SM),
            vspace(gap::XS),
            on.line(text_w * 0.4, type_scale::BODY_MD),
            vspace(gap::LG),
            on.block(Length::Fixed(112.0), 40.0, shape::FULL),
        ];
        let body = row![
            info,
            Space::new().width(Length::Fill),
            on.block(
                Length::Fixed(HERO_ART),
                HERO_ART,
                ArtShape::Tile.radius(HERO_ART)
            ),
        ]
        .align_y(Alignment::Center);
        stack![
            self.block(Length::Fixed(width), HERO_H, shape::xl()),
            container(body)
                .width(Length::Fixed(width))
                .height(Length::Fixed(HERO_H))
                .padding(gap::XL)
                .align_y(Alignment::Center),
        ]
        .into()
    }

    /// The Top result block of Search, `width` wide and [`TOP_RESULT_H`] high: the `Top result`
    /// line, then a card with a cover, title, detail line and two pills.
    #[must_use]
    pub fn top_result<'a>(self, width: f32) -> Element<'a, Message> {
        let text_w = (width - TOP_ART - gap::XL - 2.0 * gap::LG).max(TOP_ART);
        let on = self.inset();
        let info = column![
            on.line(text_w * 0.5, type_scale::TITLE_MD),
            vspace(gap::XS),
            on.line(text_w * 0.35, type_scale::BODY_MD),
            vspace(gap::LG),
            row![
                on.block(Length::Fixed(96.0), 40.0, shape::FULL),
                Space::new().width(gap::MD),
                on.block(Length::Fixed(112.0), 40.0, shape::FULL),
            ],
        ];
        let card_h = TOP_ART + 2.0 * gap::LG;
        let body = row![
            on.block(
                Length::Fixed(TOP_ART),
                TOP_ART,
                ArtShape::Card.radius(TOP_ART)
            ),
            Space::new().width(gap::XL),
            info,
        ]
        .align_y(Alignment::Center);
        column![
            self.line(80.0, type_scale::LABEL_SM),
            vspace(gap::SM),
            stack![
                self.block(Length::Fixed(width), card_h, shape::md()),
                container(body)
                    .width(Length::Fixed(width))
                    .height(Length::Fixed(card_h))
                    .padding(Padding::from(gap::LG))
                    .align_y(Alignment::Center),
            ],
            vspace(gap::XXL),
        ]
        .into()
    }
}
