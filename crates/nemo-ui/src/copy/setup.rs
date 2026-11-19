//! Wording of the first-run setup flow (startup-flow spec). The notice text itself is
//! [`super::TOS_NOTICE`] and the playback switches use the wording of `design.md` section 12
//! ([`super::SPONSORBLOCK_LABEL`], [`super::ADBLOCK_LABEL`]); nothing is repeated here.

/// Title of the first step.
pub const WELCOME_TITLE: &str = "Welcome to NemoMusic";
/// What NemoMusic is, and that it runs on this computer (`product.md`, Local only).
pub const WELCOME: [&str; 3] = [
    "NemoMusic plays music from YouTube Music and from the files on this computer, with the lyrics shown as they are sung.",
    "Your sign-in, library and history stay on this computer. There is no server of ours.",
    "A few choices follow: the look, where your music comes from, and playback. Skip any time and change everything later in Settings.",
];

/// Welcome: title of the fact that NemoMusic runs on this computer.
pub const WELCOME_LOCAL_TITLE: &str = "Runs on this computer";
/// Welcome: title of the fact about how long the setup takes.
pub const WELCOME_TIME_TITLE: &str = "About a minute";

/// Title of the look step.
pub const LOOK_TITLE: &str = "Choose a look";
/// Helper under the look title.
pub const LOOK_HELP: &str =
    "Changes apply as you pick them. You can change them later in Settings.";

/// Title of the notice step.
pub const NOTICE_TITLE: &str = "Before you start";
/// Shown instead of the checkbox once the notice is accepted.
pub const NOTICE_ACCEPTED: &str = "You accepted this notice.";
/// Under the notice buttons.
pub const NOTICE_FOOT: &str =
    "Your own music plays without this. YouTube Music waits until you continue.";

/// Title of the sources step.
pub const SOURCES_TITLE: &str = "Where your music comes from";
/// Helper under the sources title.
pub const SOURCES_HELP: &str = "Use what you like. Each can be changed later in Settings.";
/// YouTube Music card, what signing in gives.
pub const YT_BODY: &str = "Sign in to see your library, likes and recommendations. Without an account you can still search and play.";
/// YouTube Music card while the notice is not accepted.
pub const YT_NEEDS_NOTICE: &str = "Accept the notice in the previous step to use YouTube Music.";
/// YouTube Music card after `Use without an account`.
pub const YT_WITHOUT_ACCOUNT: &str = "Using YouTube Music without an account.";
/// Way forward under a failed sign-in.
pub const YT_FAILED_WAY: &str =
    "You can also use YouTube Music without an account. Nothing here blocks you.";
/// SoundCloud card, what it is (requirements, criterion 12).
pub const SC_BODY: &str = "Public tracks only. No sign-in needed.";
/// SoundCloud card, why the switch cannot be turned on yet.
pub const SC_NOT_YET: &str =
    "The SoundCloud catalog is not part of this version yet, so the switch stays off.";
/// Local music card.
pub const LOCAL_BODY: &str =
    "Play the audio files in folders on this computer, lossless files included.";
/// Local music card when the app has no folder picker.
pub const LOCAL_NO_PICKER: &str =
    "A folder picker is not built in yet. Folders are listed in settings.json (see the README).";
/// Local music card after `Skip`.
pub const LOCAL_SKIPPED: &str = "No folder added now. You can add one later.";

/// Title of the playback step.
pub const PLAYBACK_TITLE: &str = "Playback";
/// Helper under the playback title.
pub const PLAYBACK_HELP: &str = "Both can be switched off here or in Settings at any time.";

/// Title of the last step.
pub const DONE_TITLE: &str = "You are all set";
/// Last line of the last step.
pub const DONE_MORE: &str = "More options in Settings: Last.fm, Discord, Cast and the AI features stay off until you turn them on there.";

/// Helper of the `Run setup again` row in Settings.
pub const RERUN_HELP: &str =
    "Go through the first-run choices again. Nothing you set is reset, and you stay signed in.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flow_says_it_runs_here_and_names_the_soundcloud_limits() {
        assert!(WELCOME[1].contains("no server of ours"));
        assert_eq!(SC_BODY, "Public tracks only. No sign-in needed.");
        assert!(!SC_NOT_YET.to_lowercase().contains("sign in with"));
    }

    #[test]
    fn nothing_here_promises_a_sign_in_that_does_not_exist() {
        let all = [WELCOME.join(" "), SC_BODY.to_owned(), SC_NOT_YET.to_owned()].join(" ");
        assert!(!all.to_lowercase().contains("soundcloud account"));
        assert!(!all.to_lowercase().contains("lossless stream"));
    }
}
