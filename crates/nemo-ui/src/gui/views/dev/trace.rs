//! The Trace tab: provider requests, newest first, with filter chips and the fields of the
//! selected row.

use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Border, Color, Element, Length};

use super::parts::{Ink, ROW_H, UNKNOWN, cell, empty, table_row};
use crate::gui::app::dev::{DevMsg, RowKey};
use crate::gui::app::{App, Message};
use crate::gui::style::{animated, col, gap};
use crate::gui::views::kit::chip;
use crate::theme::{color, shape};
use nemo_core::dev::DevTraceRow;

const TIME_W: f32 = 92.0;
const PROVIDER_W: f32 = 124.0;
const OP_W: f32 = 104.0;
const HOST_W: f32 = 180.0;
const STATUS_W: f32 = 88.0;
const MS_W: f32 = 68.0;
const TIER_W: f32 = 84.0;

/// The most rows drawn at once; the rest is in the report.
const MAX_ROWS: usize = 300;

/// The filter bar and the body of the tab.
pub(super) fn view(app: &App) -> (Option<Element<'_, Message>>, Element<'_, Message>) {
    (Some(filter_bar(app)), body(app))
}

fn filter_bar(app: &App) -> Element<'_, Message> {
    let accent = app.accent();
    let mut chips = row![chip(
        "All providers",
        app.dev.provider.is_none(),
        accent,
        Message::Dev(DevMsg::Provider(None)),
    )]
    .spacing(gap::SM)
    .align_y(Alignment::Center);
    for p in &app.dev.snap.providers {
        let on = app.dev.provider.as_deref() == Some(p.name.as_str());
        let next = if on { None } else { Some(p.name.clone()) };
        chips = chips.push(chip(
            &p.name,
            on,
            accent,
            Message::Dev(DevMsg::Provider(next)),
        ));
    }
    // The provider chips scroll sideways when they do not fit; `Errors only` stays in view.
    let providers = iced::widget::scrollable(chips)
        .width(Length::Fill)
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new()
                .width(0)
                .scroller_width(0),
        ));
    row![
        providers,
        Space::new().width(gap::SM),
        chip(
            "Errors only",
            app.dev.errors_only,
            accent,
            Message::Dev(DevMsg::ErrorsOnly(!app.dev.errors_only)),
        ),
    ]
    .align_y(Alignment::Center)
    .into()
}

fn body(app: &App) -> Element<'_, Message> {
    let rows = app.dev.trace_rows();
    let mut list = column![header()];
    if let Some(selected) = app.dev.selected_row() {
        list = list.push(details(selected));
    }
    if rows.is_empty() {
        let text = if app.backend.dev.is_none() {
            "No developer data in this build."
        } else {
            "No provider requests yet."
        };
        return list.push(empty(text)).into();
    }
    for r in rows.into_iter().take(MAX_ROWS) {
        list = list.push(trace_row(app, r));
    }
    list.into()
}

fn header<'a>() -> Element<'a, Message> {
    table_row(vec![
        cell("Time (UTC)", TIME_W, Ink::Dim),
        cell("Provider", PROVIDER_W, Ink::Dim),
        cell("Op", OP_W, Ink::Dim),
        cell("Host", HOST_W, Ink::Dim),
        cell("Status", STATUS_W, Ink::Dim),
        cell("ms", MS_W, Ink::Dim),
        cell("Tier", TIER_W, Ink::Dim),
    ])
}

fn trace_row<'a>(app: &'a App, r: &DevTraceRow) -> Element<'a, Message> {
    let key: RowKey = (r.at_ms, r.provider.clone(), r.op.clone());
    let selected = app.dev.selected.as_ref() == Some(&key);
    let ink = if r.failed { Ink::Bad } else { Ink::Plain };
    let content = table_row(vec![
        cell(
            &crate::gui::app::dev::report::clock(r.at_ms),
            TIME_W,
            Ink::Dim,
        ),
        cell(&r.provider, PROVIDER_W, Ink::Plain),
        cell(&r.op, OP_W, Ink::Plain),
        cell(
            if r.host.is_empty() { "-" } else { &r.host },
            HOST_W,
            Ink::Dim,
        ),
        cell(&r.status, STATUS_W, ink),
        cell(
            &r.ms.map_or_else(|| "-".to_owned(), |v| v.to_string()),
            MS_W,
            Ink::Plain,
        ),
        cell(r.tier.as_deref().unwrap_or("-"), TIER_W, Ink::Dim),
    ]);
    let next = if selected { None } else { Some(key) };
    let fill = if selected {
        col(color::surface_container_highest())
    } else {
        Color::TRANSPARENT
    };
    animated(
        button(content)
            .padding(0)
            .height(Length::Fixed(ROW_H))
            .on_press(Message::Dev(DevMsg::Select(next))),
        fill,
        shape::sm(),
    )
}

/// The fields of the selected row, as `name = value` lines.
fn details<'a>(r: &DevTraceRow) -> Element<'a, Message> {
    let mut pairs = vec![
        ("provider", r.provider.clone()),
        ("op", r.op.clone()),
        ("host", r.host.clone()),
        ("status", r.status.clone()),
    ];
    for (name, value) in [
        ("ms", r.ms),
        ("bytes", r.bytes),
        ("attempt", r.attempt),
        ("rest", r.rest_s),
    ] {
        pairs.push((
            name,
            value.map_or_else(|| UNKNOWN.to_owned(), |v| v.to_string()),
        ));
    }
    pairs.push(("tier", r.tier.clone().unwrap_or_else(|| "-".to_owned())));
    let mut body = column![].spacing(gap::XS);
    for (name, value) in pairs {
        body = body.push(
            row![cell(name, 96.0, Ink::Dim), cell(&value, 360.0, Ink::Plain)].spacing(gap::SM),
        );
    }
    container(body)
        .padding(gap::MD)
        .style(|_| container::Style {
            background: Some(col(color::surface_container_highest()).into()),
            border: Border {
                radius: shape::md().into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}
