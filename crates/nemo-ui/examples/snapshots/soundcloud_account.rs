//! Shots of the SoundCloud page in Settings and the SoundCloud card of the setup flow
//! (soundcloud-account, PR 2): dark and light, 960 x 600 and 1366 x 697, with every state of the
//! account card. `Shot.soundcloud` says SoundCloud runs in the backend, with the account in that
//! state; without it the services did not start (the switch is off, or `Restart now` shows).

use nemo_core::source_account::SourceAccountState;
use nemo_ui::gui::app::SettingsSection;
use nemo_ui::gui::headless::Scenario;
use nemo_ui::gui::headless::soundcloud::ScScene;
use nemo_ui::mock::MockScAccount;
use nemo_ui::route::Route;
use nemo_ui::scheme::BaseMode;
use nemo_ui::settings::UiSettings;
use nemo_ui::setup::SetupStep;

use super::{Shot, shot};

const SMALL: (u32, u32) = (960, 600);
const LARGE: (u32, u32) = (1366, 697);

fn page(base: BaseMode, size: (u32, u32), scene: ScScene) -> Scenario {
    Scenario {
        route: Route::Settings,
        section: SettingsSection::SoundCloud,
        size,
        settings: UiSettings {
            base,
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        soundcloud: Some(scene),
        ..Scenario::default()
    }
}

fn setup(base: BaseMode, scene: ScScene) -> Scenario {
    Scenario {
        size: SMALL,
        settings: UiSettings {
            base,
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        setup: Some(SetupStep::Sources),
        stopped: Some(false),
        soundcloud: Some(scene),
        ..Scenario::default()
    }
}

/// A shot with SoundCloud running and its account in `state`.
fn running(name: String, scenario: Scenario, state: SourceAccountState) -> Shot {
    Shot {
        soundcloud: Some(state),
        ..shot(name, scenario)
    }
}

fn tag(base: BaseMode) -> &'static str {
    if base == BaseMode::Light {
        "light"
    } else {
        "dark"
    }
}

fn signed_in() -> SourceAccountState {
    SourceAccountState::SignedIn(MockScAccount::info())
}

/// Every shot of this spec.
pub(super) fn shots(out: &mut Vec<Shot>) {
    for base in [BaseMode::Dark, BaseMode::Light] {
        let t = tag(base);
        for (size, dims) in [(SMALL, "960x600"), (LARGE, "1366x697")] {
            let n = |stem: &str| format!("soundcloud-{stem}-{t}-{dims}");
            if base == BaseMode::Dark {
                out.push(shot(n("off"), page(base, size, ScScene::Off)));
            }
            out.push(running(
                n("signed-out"),
                page(base, size, ScScene::Account),
                SourceAccountState::SignedOut,
            ));
            out.push(running(
                n("signed-in"),
                page(base, size, ScScene::Account),
                signed_in(),
            ));
        }
        if base == BaseMode::Dark {
            let n = |stem: &str| format!("soundcloud-{stem}-{t}-960x600");
            out.push(shot(
                n("confirming"),
                page(base, SMALL, ScScene::Confirming),
            ));
            out.push(shot(n("restart"), page(base, SMALL, ScScene::Restart)));
            out.push(running(
                n("paste"),
                page(base, SMALL, ScScene::Paste),
                SourceAccountState::SignedOut,
            ));
            out.push(running(
                n("busy"),
                page(base, SMALL, ScScene::Busy),
                SourceAccountState::SigningIn,
            ));
            out.push(running(
                n("hint"),
                page(base, SMALL, ScScene::Hint),
                SourceAccountState::SigningIn,
            ));
            out.push(running(
                n("error"),
                page(base, SMALL, ScScene::Error),
                SourceAccountState::SignedOut,
            ));
            out.push(running(
                n("expired"),
                page(base, SMALL, ScScene::Account),
                SourceAccountState::Expired,
            ));
            out.push(running(
                n("confirm-sign-out"),
                page(base, SMALL, ScScene::ConfirmSignOut),
                signed_in(),
            ));
            out.push(running(
                format!("soundcloud-confirm-off-{t}-960x900"),
                page(base, (960, 900), ScScene::ConfirmOff),
                signed_in(),
            ));
            out.push(running(
                n("signed-out-notice"),
                page(base, SMALL, ScScene::SignedOutNotice),
                SourceAccountState::SignedOut,
            ));
        }
        // The setup flow's Sources step.
        out.push(shot(
            format!("setup-sources-soundcloud-off-{t}-960x600"),
            setup(base, ScScene::Off),
        ));
        if base == BaseMode::Dark {
            out.push(running(
                format!("setup-sources-soundcloud-hint-{t}-960x600"),
                setup(base, ScScene::Hint),
                SourceAccountState::SigningIn,
            ));
        }
        out.push(running(
            format!("setup-sources-soundcloud-sign-in-{t}-960x600"),
            setup(base, ScScene::Account),
            SourceAccountState::SignedOut,
        ));
    }
    out.push(shot(
        "setup-sources-soundcloud-confirming-dark-960x600".to_owned(),
        setup(BaseMode::Dark, ScScene::Confirming),
    ));
    out.push(running(
        "setup-sources-soundcloud-signed-in-dark-960x600".to_owned(),
        setup(BaseMode::Dark, ScScene::Account),
        signed_in(),
    ));
    out.push(shot(
        "setup-sources-soundcloud-restart-dark-960x600".to_owned(),
        setup(BaseMode::Dark, ScScene::Restart),
    ));
}

/// The SoundCloud Library (PR 4): every tab with data, loading, failed, empty, signed out and off.
pub(super) fn library_shots(out: &mut Vec<Shot>) {
    use nemo_ui::gui::app::ScTab;
    use nemo_ui::gui::headless::soundcloud::ScLibraryScene;

    let lib = |base: BaseMode, size: (u32, u32), scene: ScLibraryScene| Scenario {
        route: Route::Library,
        size,
        settings: UiSettings {
            base,
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        sc_library: Some(scene),
        ..Scenario::default()
    };
    let tab = |t: ScTab| ScLibraryScene::Tab(t);
    for (size, dims) in [(SMALL, "960x600"), (LARGE, "1366x697")] {
        for t in ScTab::ALL {
            let name = format!(
                "soundcloud-library-{}-dark-{dims}",
                t.label().to_lowercase()
            );
            out.push(running(
                name,
                lib(BaseMode::Dark, size, tab(t)),
                signed_in(),
            ));
        }
    }
    for (t, stem) in [(ScTab::Likes, "likes"), (ScTab::Playlists, "playlists")] {
        for (size, dims) in [(SMALL, "960x600"), (LARGE, "1366x697")] {
            out.push(running(
                format!("soundcloud-library-{stem}-light-{dims}"),
                lib(BaseMode::Light, size, tab(t)),
                signed_in(),
            ));
        }
    }
    let dark = |scene| lib(BaseMode::Dark, SMALL, scene);
    out.push(Shot {
        scenario: Scenario {
            skeleton_phase: Some(0.0),
            ..dark(ScLibraryScene::Loading)
        },
        ..running(
            "soundcloud-library-loading-dark-960x600".to_owned(),
            dark(ScLibraryScene::Loading),
            signed_in(),
        )
    });
    out.push(running(
        "soundcloud-library-error-dark-960x600".to_owned(),
        dark(ScLibraryScene::Failed),
        signed_in(),
    ));
    out.push(running(
        "soundcloud-library-signed-out-dark-960x600".to_owned(),
        dark(ScLibraryScene::SignedOut),
        SourceAccountState::SignedOut,
    ));
    out.push(shot(
        "soundcloud-library-off-dark-960x600".to_owned(),
        dark(ScLibraryScene::Off),
    ));
    out.push(Shot {
        empty: true,
        ..running(
            "soundcloud-library-empty-dark-960x600".to_owned(),
            dark(tab(ScTab::Likes)),
            signed_in(),
        )
    });
}

/// Search with the source switch on SoundCloud (PR 5): the answer, a filter chip, loading, and the
/// switch on YouTube Music for comparison.
pub(super) fn search_shots(out: &mut Vec<Shot>) {
    use nemo_core::catalog::SearchKind;
    use nemo_ui::gui::app::Source;
    use nemo_ui::gui::headless::Pending;
    use nemo_ui::gui::headless::soundcloud::ScScene;

    let search = |base: BaseMode, size: (u32, u32), source: Source| Scenario {
        route: Route::Search(String::new()),
        size,
        settings: UiSettings {
            base,
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        soundcloud: Some(ScScene::Account),
        area: Some(source),
        search: Some("velvet".to_owned()),
        ..Scenario::default()
    };
    for (base, tag) in [(BaseMode::Dark, "dark"), (BaseMode::Light, "light")] {
        for (size, dims) in [(SMALL, "960x600"), (LARGE, "1366x697")] {
            out.push(running(
                format!("soundcloud-search-{tag}-{dims}"),
                search(base, size, Source::SoundCloud),
                signed_in(),
            ));
        }
    }
    out.push(running(
        "soundcloud-search-songs-dark-960x600".to_owned(),
        Scenario {
            search_filter: Some(SearchKind::Songs),
            ..search(BaseMode::Dark, SMALL, Source::SoundCloud)
        },
        signed_in(),
    ));
    out.push(running(
        "soundcloud-search-playlists-dark-960x600".to_owned(),
        Scenario {
            search_filter: Some(SearchKind::Playlists),
            ..search(BaseMode::Dark, SMALL, Source::SoundCloud)
        },
        signed_in(),
    ));
    out.push(running(
        "soundcloud-search-loading-dark-960x600".to_owned(),
        Scenario {
            pending: Some(Pending::Search),
            skeleton_phase: Some(0.0),
            ..search(BaseMode::Dark, SMALL, Source::SoundCloud)
        },
        signed_in(),
    ));
    out.push(running(
        "soundcloud-search-youtube-dark-960x600".to_owned(),
        search(BaseMode::Dark, SMALL, Source::YouTube),
        signed_in(),
    ));
}

/// The writes (soundcloud-account, task 7): the track menu with its SoundCloud entries and the
/// playlist list, the name sheets, and the pages with Rename and Unfollow. Every shot is a
/// signed-in account; the library behind them is the small one of `ScWriteScene`.
pub(super) fn write_shots(out: &mut Vec<Shot>) {
    use nemo_core::catalog::BrowseId;
    use nemo_ui::gui::headless::soundcloud::{FOLLOWED_USER, OWN_PLAYLIST, ScWriteScene};

    let base = |scene: ScWriteScene, route: Route, size: (u32, u32)| Scenario {
        route,
        size,
        settings: UiSettings {
            base: BaseMode::Dark,
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        soundcloud: Some(ScScene::Account),
        sc_write: Some(scene),
        ..Scenario::default()
    };
    for (stem, scene) in [
        ("menu", ScWriteScene::Menu),
        ("pick", ScWriteScene::PickMenu),
        ("new-sheet", ScWriteScene::NewSheet),
        ("rename-sheet", ScWriteScene::RenameSheet),
    ] {
        out.push(running(
            format!("soundcloud-write-{stem}-dark-960x600"),
            base(scene, Route::Library, SMALL),
            signed_in(),
        ));
    }
    out.push(running(
        "soundcloud-write-playlist-dark-960x1300".to_owned(),
        base(
            ScWriteScene::Library,
            Route::Playlist(BrowseId(OWN_PLAYLIST.to_owned())),
            (960, 1300),
        ),
        signed_in(),
    ));
    out.push(running(
        "soundcloud-write-user-dark-960x600".to_owned(),
        base(
            ScWriteScene::Library,
            Route::Artist(BrowseId(FOLLOWED_USER.to_owned())),
            SMALL,
        ),
        signed_in(),
    ));
}

/// The sidebar source switch, the SoundCloud track page and the sort and layout of the SoundCloud
/// Library (soundcloud-account, PR 4).
pub(super) fn sidebar_shots(out: &mut Vec<Shot>) {
    use nemo_ui::explore::{LibLayout, LibSort};
    use nemo_ui::gui::app::{ScTab, Source};
    use nemo_ui::gui::headless::soundcloud::ScLibraryScene;

    let dark = |size: (u32, u32), route: Route| Scenario {
        route,
        size,
        settings: UiSettings {
            base: BaseMode::Dark,
            tos_acknowledged: true,
            ..UiSettings::default()
        },
        ..Scenario::default()
    };
    // The sidebar list in the SoundCloud area: its own rows, none of YouTube Music's.
    out.push(running(
        "soundcloud-sidebar-dark-1366x697".to_owned(),
        Scenario {
            sc_library: Some(ScLibraryScene::Tab(ScTab::Playlists)),
            area: Some(Source::SoundCloud),
            ..dark(LARGE, Route::Library)
        },
        signed_in(),
    ));
    // Sorted by title and as a grid.
    out.push(running(
        "soundcloud-library-playlists-grid-title-dark-960x600".to_owned(),
        Scenario {
            sc_library: Some(ScLibraryScene::Tab(ScTab::Playlists)),
            lib_layout: LibLayout::Grid,
            lib_sort: LibSort::Title,
            ..dark(SMALL, Route::Library)
        },
        signed_in(),
    ));
    // A track page: the track, then what SoundCloud suggests.
    out.push(running(
        "soundcloud-track-page-dark-960x1000".to_owned(),
        Scenario {
            soundcloud: Some(ScScene::Account),
            ..dark(
                (960, 1000),
                Route::Playlist(nemo_core::catalog::BrowseId("sc:track:5000002".to_owned())),
            )
        },
        signed_in(),
    ));
}
