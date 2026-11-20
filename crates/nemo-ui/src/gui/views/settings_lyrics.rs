//! Lyrics sources: Settings > Lyrics (wave-lyrics criterion 11), laid out like the Better
//! Lyrics "Sources" screen: every provider sorted by the finest timing it can give (syllable,
//! word, line, then chain order), with a level tag, its hosts, its risk note and an on/off
//! switch. Also the "Lyrics source" section of the Tools panel (criterion 10).

use iced::font::Weight;
use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Border, Color, Element, Length, Padding};
use nemo_core::services::LyricsProviderStatus;

use super::super::app::{App, Message};
use super::super::assets::Icon;
use super::super::style::{animated, col, fit, gap, icon, label, label_w};
use super::kit::{paragraph, vspace};
use super::settings::switch_with;
use crate::lyrics_view::level_tag;
use crate::theme::{color, shape, type_scale};

/// A thin section label (as the other Tools panel sections).
fn heading(text: &str) -> Element<'_, Message> {
    label_w(
        text,
        type_scale::LABEL_MD,
        Weight::Medium,
        col(color::on_surface_variant()),
    )
    .into()
}

/// Width kept for the switch and its On/Off text.
const CONTROL_W: f32 = 120.0;

/// The providers, best timing first; equal levels keep the chain order (a stable sort).
#[must_use]
pub fn sorted(sources: &[LyricsProviderStatus]) -> Vec<&LyricsProviderStatus> {
    let mut out: Vec<&LyricsProviderStatus> = sources.iter().collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.best));
    out
}

/// The helper text of one source: hosts, default state and risk note.
#[must_use]
pub fn helper(s: &LyricsProviderStatus) -> String {
    let mut text = format!(
        "Asks {}. {}",
        s.hosts.join(", "),
        if s.default_enabled {
            "On by default."
        } else {
            "Off until you turn it on."
        }
    );
    if let Some(risk) = s.risk_note {
        text = format!("{text} Risk: {risk}");
    }
    text
}

/// Localized helper text of one source: hosts, default state and risk note.
#[must_use]
pub fn helper_localized(s: &LyricsProviderStatus, lang: crate::settings::Language) -> String {
    let mut text = match lang {
        crate::settings::Language::Vietnamese => format!(
            "Truy vấn {}. {}",
            s.hosts.join(", "),
            if s.default_enabled {
                "Bật mặc định."
            } else {
                "Tắt cho đến khi bạn bật."
            }
        ),
        crate::settings::Language::English => format!(
            "Asks {}. {}",
            s.hosts.join(", "),
            if s.default_enabled {
                "On by default."
            } else {
                "Off until you turn it on."
            }
        ),
    };
    if let Some(risk) = s.risk_note {
        let risk_word = crate::i18n::t(lang, "Risk", "Rủi ro");
        text = format!("{text} {risk_word}: {risk}");
    }
    text
}

fn tag<'a>(text: &'static str) -> Element<'a, Message> {
    let ink = col(color::on_surface_variant());
    container(label(text, type_scale::LABEL_SM, false, ink))
        .padding(Padding::from([0.0, gap::SM]))
        .style(move |_| container::Style {
            border: Border {
                radius: shape::FULL.into(),
                width: 1.0,
                color: col(color::outline()),
            },
            ..container::Style::default()
        })
        .into()
}

fn setting_source_row<'a>(
    s: &LyricsProviderStatus,
    lang: crate::settings::Language,
    width: f32,
) -> Element<'a, Message> {
    let text_w = (width - CONTROL_W - 3.0 * gap::LG).max(160.0);
    let id = s.id;
    let title = row![
        label(
            id.as_str(),
            type_scale::BODY_MD,
            false,
            col(color::on_surface())
        ),
        Space::new().width(gap::SM),
        tag(level_tag(s.best)),
    ]
    .align_y(Alignment::Center);
    let text = column![
        title,
        paragraph(
            helper_localized(s, lang),
            type_scale::BODY_SM,
            col(color::on_surface_variant()),
            text_w
        ),
    ]
    .spacing(gap::XS)
    .width(Length::Fixed(text_w));
    row![
        Space::new().height(32.0),
        text,
        Space::new().width(Length::Fill),
        container(switch_with(lang, s.enabled, move |v| Message::SetLyricsSource(
            id, v
        )))
        .width(Length::Fixed(CONTROL_W))
        .align_x(Alignment::End),
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([gap::MD, gap::LG]))
    .into()
}

/// The intro line and one row per source.
pub fn sources(app: &App, width: f32) -> Vec<Element<'_, Message>> {
    let lang = app.ui.settings.language;
    let intro_text = crate::i18n::t(
        lang,
        "Sources are asked best timing first: syllable, then word, then line. The best answer is shown by itself; pick another one for a song in the Tools panel.",
        "Các nguồn được ưu tiên theo độ khớp thời gian tốt nhất: từng âm tiết, từng từ, rồi đến từng câu. Kết quả tốt nhất được hiển thị tự động; bạn có thể chọn nguồn khác cho bài hát trong bảng Công cụ.",
    );
    let intro = container(paragraph(
        intro_text,
        type_scale::BODY_SM,
        col(color::on_surface_variant()),
        (width - 2.0 * gap::LG).max(160.0),
    ))
    .padding(Padding::from([gap::MD, gap::LG]));
    std::iter::once(intro.into())
        .chain(
            sorted(&app.lyric_sources)
                .into_iter()
                .map(|s| setting_source_row(s, lang, width)),
        )
        .collect()
}

/// A lyrics source row of the Tools panel: filled when it is the chosen setting, a check when
/// its lyrics are the ones shown, the name and the level tag. 40 px high, a focusable button.
fn source_row<'a>(
    app: &App,
    name: String,
    tag: &'static str,
    (selected, in_use): (bool, bool),
    message: Message,
) -> Element<'a, Message> {
    let accent = app.accent();
    let (fill, ink) = if selected {
        (
            col(accent.secondary_container),
            col(accent.on_secondary_container),
        )
    } else {
        (Color::TRANSPARENT, col(color::on_surface()))
    };
    let mark: Element<'a, Message> = if in_use {
        icon(Icon::Check, 20.0, ink).into()
    } else {
        Space::new().width(20.0).into()
    };
    let tag_pill = container(label(tag, type_scale::LABEL_SM, false, ink))
        .padding(Padding::from([0.0, gap::SM]))
        .style(move |_| container::Style {
            border: Border {
                radius: shape::FULL.into(),
                width: 1.0,
                color: ink.scale_alpha(0.5),
            },
            ..container::Style::default()
        });
    let r = row![
        mark,
        Space::new().width(gap::MD),
        label(name, type_scale::BODY_MD, false, ink),
        Space::new().width(Length::Fill),
        tag_pill,
    ]
    .align_y(Alignment::Center)
    .padding(Padding::from([0.0, gap::MD]))
    .height(Length::Fixed(40.0));
    animated(
        button(r).padding(0).width(Length::Fill).on_press(message),
        fill,
        shape::md(),
    )
}

/// "Lyrics source" in the Tools panel (wave-lyrics criterion 10): `Best (auto)` forgets the
/// per-song pick, then every source that answered for this song, best first, with its level.
/// Empty while no source answered.
pub fn tools_section(app: &App, width: f32) -> Option<Element<'_, Message>> {
    let lang = app.ui.settings.language;
    let best = app.lyric_candidates.first()?;
    let section_title = crate::i18n::t(lang, "Lyrics source", "Nguồn lời bài hát");
    let best_label = crate::i18n::t(lang, "Best (auto)", "Tự động (tốt nhất)");
    let mut c = column![heading(section_title), vspace(gap::SM)].width(Length::Fixed(width));
    c = c.push(source_row(
        app,
        best_label.to_owned(),
        level_tag(best.granularity),
        (!app.lyric_pinned, false),
        Message::LyricsAuto,
    ));
    for (i, cand) in app.lyric_candidates.iter().enumerate() {
        let name = fit(
            cand.provider.as_str(),
            width * 0.55,
            type_scale::BODY_MD.size,
        );
        let shown = i == app.lyric_provider;
        c = c.push(source_row(
            app,
            name,
            level_tag(cand.granularity),
            (app.lyric_pinned && shown, shown),
            Message::PickLyricsProvider(i),
        ));
    }
    let note = if app.lyric_pinned {
        crate::i18n::t(
            lang,
            "Your pick is kept for this song.",
            "Lựa chọn của bạn được lưu cho bài hát này.",
        )
    } else {
        crate::i18n::t(
            lang,
            "The best timing is picked by itself: syllable, then word, then line.",
            "Độ khớp thời gian tốt nhất được chọn tự động: từng âm tiết, từng từ, rồi đến từng câu.",
        )
    };
    Some(
        c.push(vspace(gap::XS))
            .push(paragraph(
                note,
                type_scale::BODY_SM,
                col(color::on_surface_variant()),
                width,
            ))
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_core::lyrics::{Granularity, ProviderId};

    fn status(name: &'static str, best: Granularity) -> LyricsProviderStatus {
        LyricsProviderStatus {
            id: ProviderId::new(name),
            enabled: true,
            default_enabled: true,
            risk_note: None,
            hosts: &["x.invalid"],
            best,
        }
    }

    #[test]
    fn sources_sort_by_level_and_keep_chain_order_within_one() {
        let list = [
            status("A", Granularity::Line),
            status("B", Granularity::Syllable),
            status("C", Granularity::Word),
            status("D", Granularity::Line),
            status("E", Granularity::Syllable),
        ];
        let names: Vec<&str> = sorted(&list).iter().map(|s| s.id.as_str()).collect();
        assert_eq!(names, ["B", "E", "C", "A", "D"]);
    }

    /// The row a grey syllable source gets (`LyricsPlus`, syllable-lyrics spec): listed with
    /// the other syllable sources, off until the user turns it on, its host and risk named.
    #[test]
    fn grey_syllable_source_row_sorts_by_level_and_says_it_is_off() {
        let mut grey = status("LyricsPlus", Granularity::Syllable);
        grey.enabled = false;
        grey.default_enabled = false;
        grey.hosts = &["lyricsplus.prjktla.my.id"];
        grey.risk_note = Some("Unofficial relay.");
        let list = [
            status("LRCLIB", Granularity::Line),
            status("Unison", Granularity::Syllable),
            grey,
            status("NetEase", Granularity::Line),
        ];
        let names: Vec<&str> = sorted(&list).iter().map(|s| s.id.as_str()).collect();
        assert_eq!(names, ["Unison", "LyricsPlus", "LRCLIB", "NetEase"]);
        assert_eq!(
            helper(&list[2]),
            "Asks lyricsplus.prjktla.my.id. Off until you turn it on. Risk: Unofficial relay."
        );
    }

    #[test]
    fn helper_names_hosts_default_and_risk() {
        let mut s = status("A", Granularity::Line);
        s.default_enabled = false;
        s.risk_note = Some("Unofficial.");
        assert_eq!(
            helper(&s),
            "Asks x.invalid. Off until you turn it on. Risk: Unofficial."
        );
    }
}
