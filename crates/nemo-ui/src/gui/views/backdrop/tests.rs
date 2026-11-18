//! Tests of the window backdrop with the mock backend (no renderer needed).

use nemo_core::catalog::BrowseId;
use nemo_core::player::PlayerCommand;

use super::window_inputs;
use crate::drift::Mode;
use crate::gui::app::{App, Message};
use crate::gui::headless::{Scenario, build_app};
use crate::mock::mock_backend;
use crate::route::Route;
use crate::scheme::BaseMode;
use crate::state::Event;

fn app_at(route: Route, track: usize) -> App {
    build_app(
        &mock_backend(),
        &Scenario {
            route,
            track,
            playing: true,
            ..Scenario::default()
        },
    )
}

/// Play mock song 9, whose cover is violet where song 1's is red.
fn play_other(app: &mut App) {
    send(
        app,
        Message::Ui(Event::Command(PlayerCommand::Play(crate::mock::track(9)))),
    );
}

fn send(app: &mut App, message: Message) {
    let jobs = app.update_core(message);
    app.settle_blocking(jobs);
    app.finish_animations();
}

#[test]
fn a_track_change_rebuilds_the_backdrop_from_the_new_cover() {
    let mut app = app_at(Route::Home, 1);
    let first = app.cover.clone().expect("the mock song has a cover");
    let field_before = app.bg_fade.current();
    play_other(&mut app);
    let now = app.ui.snapshot.current.clone().expect("a song plays");
    assert_ne!(now.id, first.track, "the player moved on");
    let second = app.cover.clone().expect("the next mock song has a cover");
    assert_eq!(
        second.track, now.id,
        "the backdrop is built from the new cover"
    );
    assert_eq!(
        app.prev_cover.as_ref().map(|c| c.track.clone()),
        Some(first.track.clone()),
        "the old cover stays under the new one for the cross-fade"
    );
    assert_ne!(
        first.canvas.stops(BaseMode::Dark),
        second.canvas.stops(BaseMode::Dark),
        "a different cover gives a different canvas"
    );
    assert_ne!(
        field_before,
        app.bg_fade.current(),
        "the fallback field moved too"
    );
}

#[test]
fn an_open_album_page_does_not_hold_the_window_colour() {
    let album = Route::Album(BrowseId("mock-album-4".to_owned()));
    let mut home = app_at(Route::Home, 1);
    let mut page = app_at(album, 1);
    home.finish_animations();
    page.finish_animations();
    assert_eq!(home.bg_fade.current(), page.bg_fade.current());
    let before = page.bg_fade.current();
    play_other(&mut page);
    assert_ne!(before, page.bg_fade.current(), "the page follows the song");
}

#[test]
fn the_backdrop_is_still_flat_or_moving_as_the_settings_say() {
    let mut app = app_at(Route::Home, 1);
    // `build_app` turns reduced motion on for stable shots.
    assert_eq!(Mode::of(window_inputs(&app)), Mode::Still);
    app.ui.reduced_motion = false;
    assert_eq!(Mode::of(window_inputs(&app)), Mode::Moving);
    app.ui.settings.reduce_transparency = true;
    assert_eq!(Mode::of(window_inputs(&app)), Mode::Flat);
    app.ui.settings.reduce_transparency = false;
    send(&mut app, Message::Ui(Event::Command(PlayerCommand::Pause)));
    app.ui.reduced_motion = false;
    assert_eq!(Mode::of(window_inputs(&app)), Mode::Still, "paused");
    send(&mut app, Message::Ui(Event::Command(PlayerCommand::Resume)));
    app.ui.reduced_motion = false;
    send(&mut app, Message::Ui(Event::Go(Route::NowPlaying)));
    app.ui.reduced_motion = false;
    assert_eq!(Mode::of(window_inputs(&app)), Mode::Still, "hidden");
}

#[test]
fn the_song_switch_decides_where_the_window_colours_come_from() {
    use crate::palette::Palette;
    use crate::settings::SettingChange;
    let mut app = app_at(Route::Home, 1);
    app.finish_animations();
    let tinted = app.scheme_fade.current().container;
    assert_ne!(
        tinted,
        BaseMode::Dark.neutrals().container,
        "panes take the song tint"
    );
    assert!(!app.ui.accent.current().is_fallback);
    send(
        &mut app,
        Message::Ui(Event::Setting(SettingChange::SongColourTheme(false))),
    );
    assert_eq!(
        app.scheme_fade.current().container,
        BaseMode::Dark.neutrals().container,
        "off: neutral panes"
    );
    assert_eq!(
        app.ui.accent.current(),
        Palette::fallback(),
        "off: Dynamic has no song"
    );
    send(
        &mut app,
        Message::Ui(Event::Setting(SettingChange::SongColourTheme(true))),
    );
    assert_eq!(app.scheme_fade.current().container, tinted, "on again");
}
