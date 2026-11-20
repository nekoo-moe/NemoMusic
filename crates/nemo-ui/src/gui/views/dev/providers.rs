//! The Providers tab: one card per provider, then the stream resolver.

use iced::widget::column;
use iced::{Element, Length};

use super::parts::{Ink, UNKNOWN, card, card_two, empty, title};
use crate::gui::app::dev::report::{ms, path_kind, solver, when};
use crate::gui::app::{App, Message};
use crate::gui::style::gap;
use nemo_core::dev::{DevProvider, DevResolver};

/// The body of the tab.
pub(super) fn view(app: &App) -> Element<'_, Message> {
    let width = super::inner_width(app);
    let snap = &app.dev.snap;
    let mut list = column![].spacing(gap::SM).width(Length::Fill);
    if snap.providers.is_empty() {
        list = list.push(empty("No provider requests yet."));
    }
    for p in &snap.providers {
        list = list.push(provider_card(p, snap.taken_ms, width));
    }
    list = list.push(title("Stream resolver"));
    list = list.push(resolver_card(&snap.resolver, width));
    list.into()
}

fn provider_card<'a>(p: &DevProvider, now_ms: u64, width: f32) -> Element<'a, Message> {
    let rest = match p.resting_until_ms {
        Some(until) if until > now_ms => format!("{} s left", (until - now_ms).div_ceil(1000)),
        _ => "none".to_owned(),
    };
    let error_ink = if p.errors > 0 { Ink::Bad } else { Ink::Plain };
    card_two(
        &p.name,
        vec![
            ("Calls".to_owned(), p.calls.to_string(), Ink::Plain),
            ("Errors".to_owned(), p.errors.to_string(), error_ink),
            ("Last success".to_owned(), when(p.last_ok_ms), Ink::Plain),
            (
                "Last error".to_owned(),
                p.last_error_kind.clone().unwrap_or_else(|| "-".to_owned()),
                error_ink,
            ),
            ("Median, last 50".to_owned(), ms(p.p50_ms), Ink::Plain),
            ("95th percentile".to_owned(), ms(p.p95_ms), Ink::Plain),
            ("Rest or backoff".to_owned(), rest, Ink::Plain),
        ],
        width,
    )
}

fn resolver_card<'a>(r: &DevResolver, width: f32) -> Element<'a, Message> {
    let mut lines: Vec<(String, String, Ink)> = r
        .tiers
        .iter()
        .map(|t| {
            let ink = if t.failed > 0 { Ink::Bad } else { Ink::Plain };
            (
                format!("Tier {}", t.name),
                format!("{} ok, {} failed", t.ok, t.failed),
                ink,
            )
        })
        .collect();
    let lookups = r.cache_hits + r.cache_disk_hits + r.cache_misses;
    let rate = ((r.cache_hits + r.cache_disk_hits) * 100)
        .checked_div(lookups)
        .map_or_else(|| "-".to_owned(), |percent| format!("{percent}%"));
    lines.extend([
        (
            "In-process solver".to_owned(),
            solver(r.solver).to_owned(),
            Ink::Plain,
        ),
        (
            "Stream cache".to_owned(),
            format!(
                "size {}, hit rate {rate} ({} memory, {} disk, {} miss)",
                r.cache_size
                    .map_or_else(|| UNKNOWN.to_owned(), |n| n.to_string()),
                r.cache_hits,
                r.cache_disk_hits,
                r.cache_misses
            ),
            Ink::Plain,
        ),
        (
            "yt-dlp from".to_owned(),
            path_kind(r.ytdlp_path).to_owned(),
            Ink::Plain,
        ),
        (
            "yt-dlp version".to_owned(),
            r.ytdlp_version
                .clone()
                .unwrap_or_else(|| UNKNOWN.to_owned()),
            Ink::Plain,
        ),
        (
            "yt-dlp build".to_owned(),
            r.ytdlp_build.clone().unwrap_or_else(|| UNKNOWN.to_owned()),
            Ink::Plain,
        ),
    ]);
    card("Resolver", lines, width)
}
