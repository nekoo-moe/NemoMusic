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
