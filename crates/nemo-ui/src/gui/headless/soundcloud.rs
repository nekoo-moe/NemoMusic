//! Snapshot scenes of the SoundCloud page and card (soundcloud-account spec). Each scene is
//! reached with the real messages of the page, so a picture shows what the app would show.

use nemo_core::account::SignOutReport;
use nemo_core::secret::Secret;
use nemo_core::source_account::SourceAccountError;

use super::run;
use crate::gui::app::{App, Message, ScMsg, ScTab, Source, UiOptions};
use crate::settings::OptInState;

/// Which state of the SoundCloud page a snapshot shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScScene {
    /// The switch is off.
    Off,
    /// The switch was pressed and the panel waits for "Turn on" or "Cancel".
    Confirming,
    /// The switch is on but the services start with the next launch (`Restart now`).
    Restart,
    /// The switch is on and SoundCloud runs; the card shows the account state of the backend.
    Account,
    /// With the masked token field open and a typed (hidden) value.
    Paste,
    /// A sign-in call is running.
    Busy,
    /// The sign-in window found no session for a while (the backend's account is `SigningIn`).
    Hint,
    /// A failed sign-in and its plain-words error.
    Error,
    /// Sign out waits for its confirmation.
    ConfirmSignOut,
    /// The switch was turned off while signed in: the question waits for its answer.
    ConfirmOff,
    /// The line after a finished sign-out.
    SignedOutNotice,
}

/// What the SoundCloud switch shows in the snapshots: worded like the real one in
/// `nemo-soundcloud` (the UI cannot depend on that crate).
#[must_use]
pub fn sample_info() -> nemo_core::opt_in::OptInInfo {
    nemo_core::opt_in::OptInInfo {
        id: "soundcloud",
        title: "SoundCloud",
        sends: "Your search text, the SoundCloud pages and tracks you open or play, and, when you sign in, requests for your own SoundCloud account.",
        hosts: &[
            "soundcloud.com",
            "api-v2.soundcloud.com",
            "a-v2.sndcdn.com",
            "i1.sndcdn.com",
            "playback.media-streaming.soundcloud.cloud",
            "*.sndcdn.com",
        ],
        risk: "SoundCloud is not an official integration. NemoMusic uses the same web calls as soundcloud.com, without an official API key. SoundCloud may block this, may restrict your account if you sign in, and its terms may not allow it. When you sign in, a token for your SoundCloud session is kept in your computer's credential store and is sent only to SoundCloud. Plays here may not count in artists' SoundCloud statistics. SoundCloud audio is lossy (AAC or MP3). Off by default.",
        version: 2,
        default_on: false,
    }
}

/// The options of a scene: the switch info, and the switch on and confirmed unless the scene is
/// `Off` or `Confirming`.
pub(super) fn options(scene: ScScene, options: &mut UiOptions) {
    let info = sample_info();
    options.soundcloud_info = Some(info);
    if !matches!(scene, ScScene::Off | ScScene::Confirming) {
        options.settings.soundcloud = OptInState {
            enabled: true,
            confirmed_version: info.version,
        };
    }
}

/// Bring the page of a started app into `scene`.
pub(super) fn enter(app: &mut App, scene: ScScene) {
    match scene {
        ScScene::Off | ScScene::Restart | ScScene::Account => {}
        ScScene::Confirming => run(app, Message::Sc(ScMsg::Ask)),
        ScScene::Paste => {
            run(app, Message::Sc(ScMsg::PasteMode(true)));
            run(
                app,
                Message::Sc(ScMsg::PasteInput(Secret::new(
                    "2-293232-7000001-x".to_owned(),
                ))),
            );
        }
        ScScene::Busy => app.sc.busy = true,
        ScScene::Hint => {
            app.sc.busy = true;
            app.sc.window = true;
        }
        ScScene::Error => run(
            app,
            Message::Sc(ScMsg::SignedIn(Err(SourceAccountError::Rejected))),
        ),
        ScScene::ConfirmSignOut => run(app, Message::Sc(ScMsg::AskSignOut)),
        ScScene::ConfirmOff => run(app, Message::Sc(ScMsg::TurnOff)),
        ScScene::SignedOutNotice => run(
            app,
            Message::Sc(ScMsg::SignedOut(Ok(SignOutReport {
                credentials_removed: 1,
                files_removed: 2,
                items_removed: 14,
                ..SignOutReport::default()
            }))),
        ),
    }
}

/// Which state of the SoundCloud writes a snapshot shows (soundcloud-account, task 7). The scene
/// holds a small library: one playlist of the user's own, one liked, one followed user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScWriteScene {
    /// The library only: the page of the scenario's route shows its write buttons.
    Library,
    /// The track menu of a SoundCloud track.
    Menu,
    /// The track menu listing the playlists to add the track to.
    PickMenu,
    /// The sheet that names a new playlist.
    NewSheet,
    /// The sheet that renames the user's playlist.
    RenameSheet,
}

/// The page id of the user's own playlist in the write scenes. It is an id the mock catalog
/// knows, so the page loads; the real ids are `sc:playlist:<number>`.
pub const OWN_PLAYLIST: &str = "mock-playlist-0";
/// The page id of the followed user in the write scenes (a mock catalog id, too).
pub const FOLLOWED_USER: &str = "mock-artist-0";

/// A SoundCloud track of the mock catalog.
fn sc_track() -> nemo_core::track::Track {
    let mut track = crate::mock::track(0);
    track.id = nemo_core::track::TrackId("sc:5000002".to_owned());
    track.origin = nemo_core::audio::Origin::SoundCloud;
    track.video_id = None;
    track
}

/// Bring a started app into `scene`.
pub(super) fn enter_write(app: &mut App, scene: ScWriteScene) {
    use nemo_core::catalog::{ArtistSummary, BrowseId, PlaylistSummary};
    let list = |id: &str, title: &str| PlaylistSummary {
        id: BrowseId(id.to_owned()),
        title: title.to_owned(),
        author: None,
        thumbnails: Vec::new(),
    };
    app.sc_lib.data = std::sync::Arc::new(crate::gui::app::ScLibData {
        playlists: vec![
            list(OWN_PLAYLIST, "Night drive"),
            list("mock-playlist-1", "Slow mornings"),
            list("mock-playlist-2", "Someone else's mix"),
        ],
        own_playlists: 2,
        artists: vec![ArtistSummary {
            id: BrowseId(FOLLOWED_USER.to_owned()),
            name: "Velvet Harbor".to_owned(),
            thumbnails: Vec::new(),
        }],
        ..crate::gui::app::ScLibData::default()
    });
    app.sc_lib.phase = crate::gui::app::ScPhase::Done;
    app.sc_lib.owner = true;
    let open = |app: &mut App| {
        run(app, Message::Ui(crate::state::Event::OpenMenu(sc_track())));
    };
    match scene {
        ScWriteScene::Library => {}
        ScWriteScene::Menu => open(app),
        ScWriteScene::PickMenu => {
            open(app);
            run(
                app,
                Message::ScWrite(crate::gui::app::ScWriteMsg::PickPlaylist(true)),
            );
        }
        ScWriteScene::NewSheet => run(
            app,
            Message::ScWrite(crate::gui::app::ScWriteMsg::NewPlaylist(None)),
        ),
        ScWriteScene::RenameSheet => run(
            app,
            Message::ScWrite(crate::gui::app::ScWriteMsg::Rename(BrowseId(
                OWN_PLAYLIST.to_owned(),
            ))),
        ),
    }
}

/// Which state of the SoundCloud Library a snapshot shows (soundcloud-account, PR 4). The backend
/// of the snapshot decides whether SoundCloud runs and whether the account is signed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScLibraryScene {
    /// The tab with its data.
    Tab(ScTab),
    /// The first read is on its way (skeleton).
    Loading,
    /// The first read failed.
    Failed,
    /// Signed out (the backend's account is signed out).
    SignedOut,
    /// SoundCloud is switched off.
    Off,
}

/// The options of a library scene: SoundCloud on, except for `Off`.
pub(super) fn library_options(scene: ScLibraryScene, opts: &mut UiOptions) {
    let base = if scene == ScLibraryScene::Off {
        ScScene::Off
    } else {
        ScScene::Account
    };
    options(base, opts);
}

/// Show the SoundCloud Library in `scene` on a started app.
pub(super) fn enter_library(app: &mut App, scene: ScLibraryScene) {
    run(app, Message::Sc(ScMsg::Source(Source::SoundCloud)));
    match scene {
        ScLibraryScene::Tab(tab) => run(app, Message::Sc(ScMsg::Tab(tab))),
        ScLibraryScene::Loading => app.sc_lib.phase = crate::gui::app::ScPhase::Loading,
        ScLibraryScene::Failed => {
            let generation = app.sc_lib.generation;
            run(
                app,
                Message::Sc(ScMsg::LibraryLoaded {
                    generation,
                    result: Err("could not reach SoundCloud".to_owned()),
                }),
            );
            // A first read that failed has nothing to show.
            app.sc_lib.data = std::sync::Arc::default();
        }
        ScLibraryScene::SignedOut | ScLibraryScene::Off => {}
    }
}
