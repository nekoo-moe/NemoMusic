//! Wording of the SoundCloud page and card (soundcloud-account spec, ADR 0005). Plain words:
//! the access is unofficial, the token stays on this computer, the audio is lossy and plays are
//! not counted for artists. Nothing here names a token value, a user name or an id.

use nemo_core::account::SignOutReport;
use nemo_core::source_account::SourceAccountError;
use nemo_core::source_writes::WriteError;

/// Title of the SoundCloud section and card.
pub const TITLE: &str = "SoundCloud";

/// The account card, signed out: what signing in gives.
pub const SIGNED_OUT_BODY: &str = "Sign in to see your likes, playlists, the artists you follow and your history from SoundCloud. Without an account you can still search and play public tracks.";
/// The account card: where the token lives.
pub const LOCAL: &str = concat!(
    "NemoMusic opens SoundCloud\u{2019}s own sign-in page and never sees your password. A token for your session is kept in ",
    store!(),
    " on this computer and is sent only to SoundCloud."
);
/// The account card: the honest short list.
pub const HONEST: &str = "This is unofficial access. SoundCloud may block it or restrict your account, and its terms may not allow it. SoundCloud audio is lossy (MP3 or AAC). Plays here are not counted for artists.";
/// Primary button.
pub const SIGN_IN: &str = "Sign in with SoundCloud";
/// Fallback toggle.
pub const PASTE: &str = "Paste a token instead";
/// Back from the fallback.
pub const USE_WINDOW: &str = "Use the sign-in window instead";
/// Placeholder of the masked field.
pub const PLACEHOLDER: &str = "Paste the oauth_token value here";
/// Button that checks a pasted token.
pub const CHECK_TOKEN: &str = "Check token";
/// Help under the masked field, in order of what the person does.
pub const PASTE_HELP: &str = concat!(
    "In your own browser, sign in at soundcloud.com. Open the developer tools (F12), then Application (Chrome, Edge) or Storage (Firefox), then Cookies, then soundcloud.com. Copy the value of the cookie named oauth_token and paste it here. ",
    "Anyone who has this value can use your SoundCloud account, so share it with no one. It is hidden while you type and kept only in ",
    store!(),
    "."
);
/// Waiting for the sign-in window.
pub const WAITING: &str =
    "Finish signing in in the window that opened. This page updates by itself.";
/// The sign-in window found no session for a while.
pub const STILL_WAITING: &str = "SoundCloud has not given this window a sign-in yet. If the window already shows you signed in, press the button. You can also paste a token instead.";
/// The same, on the setup card, which has no token field.
pub const SETUP_STILL_WAITING: &str = "SoundCloud has not given the window a sign-in yet. If the window already shows you signed in, press the button. You can also finish this later in Settings.";
/// Button that makes the window look for the session now.
pub const CHECK_AGAIN: &str = "I am signed in, check again";
/// After "check again" found nothing.
pub const STILL_NOTHING: &str = "I looked again and found no sign-in. Paste a token instead, or close the window and press Sign in with SoundCloud once more.";
/// Checking a pasted token.
pub const CHECKING: &str = "Checking with SoundCloud\u{2026}";
/// Signing out takes a moment.
pub const SIGNING_OUT: &str = "Signing out\u{2026}";
/// Sign-out confirmation.
pub const SIGN_OUT_CONFIRM: &str = "This removes your SoundCloud token and the SoundCloud data NemoMusic loaded for you from this computer. It does not sign you out of soundcloud.com in your browser.";
/// The session ended on SoundCloud's side.
pub const EXPIRED: &str =
    "SoundCloud ended your sign-in. Sign in to SoundCloud again. Public tracks still play.";
/// No credential store on this computer.
pub const NO_STORE: &str = concat!(
    "Sign-in is off because this computer has no ",
    store!(),
    " to keep the token in. Nothing is saved."
);
/// Switched on, but the services start with the app.
pub const STARTS_AFTER_RESTART: &str = "SoundCloud starts the next time NemoMusic starts.";
/// Button that restarts the app.
pub const RESTART: &str = "Restart now";
/// Status line of the switch while off.
pub const STATUS_OFF: &str = "Off. Nothing is requested from SoundCloud.";
/// Status line while on and signed in (the card shows the name).
pub const STATUS_SIGNED_IN: &str = "On. You are signed in.";
/// Status line while on and signed out.
pub const STATUS_SIGNED_OUT: &str = "On. You are not signed in.";
/// What stays on this computer while off, when nothing says whether a token was kept.
pub const STAYS: &str = "A token you signed in with earlier stays in the credential store until you sign out. To remove it, turn SoundCloud on and sign out.";
/// What stays on this computer while off, with a sign-in kept in this session.
pub const STAYS_SIGNED_IN: &str =
    "Your SoundCloud token stays in the credential store until you press Delete now.";
/// The section when this build has no SoundCloud.
pub const NOT_IN_THIS_BUILD: &str = "SoundCloud is not part of this build.";

/// Setup flow, SoundCloud card, what it is.
pub const SETUP_BODY: &str = "Search and play SoundCloud next to YouTube Music, and sign in to see your own library. It is unofficial: SoundCloud may block it, its audio is lossy, and plays are not counted for artists.";
/// Setup flow, after the switch was turned on in an app that has no SoundCloud running yet.
pub const SETUP_AFTER_RESTART: &str =
    "SoundCloud is on. It starts the next time NemoMusic starts; then sign in from Settings.";
/// Setup flow, button that turns the switch on.
pub const SETUP_TURN_ON: &str = "Turn on SoundCloud";

/// Plain words for a sign-in failure. Never contains a token.
#[must_use]
pub fn error(error: &SourceAccountError) -> String {
    use SourceAccountError as E;
    match error {
        E::Cancelled => "The sign-in window was closed before you finished. You are still signed out.".to_owned(),
        E::Rejected => "SoundCloud did not accept this sign-in. If you pasted a token, check that you copied the whole oauth_token value from a browser where you are signed in.".to_owned(),
        E::Unavailable(_) => "The sign-in window cannot open on this computer. Use \u{201c}Paste a token instead\u{201d} below.".to_owned(),
        E::Storage(_) => concat!("NemoMusic could not save the sign-in in ", store!(), ", so you are still signed out.").to_owned(),
        E::Network(_) => "The network request failed, or SoundCloud asked for a pause. Try again in a minute.".to_owned(),
    }
}

/// A like was tried without a signed-in SoundCloud account.
pub const SIGN_IN_TO_LIKE: &str = "Sign in to SoundCloud in Settings to like tracks.";
/// A playlist change was tried without a signed-in SoundCloud account.
pub const SIGN_IN_TO_WRITE: &str = "Sign in to SoundCloud in Settings to change your playlists.";
/// A second change was started while the first one waits for SoundCloud.
pub const WRITE_BUSY: &str =
    "SoundCloud is still working on your last change. Try again in a moment.";
/// Under the name field of a new SoundCloud playlist.
pub const NEW_PLAYLIST_HINT: &str = "It is private. You can change that on soundcloud.com.";
/// The unfollow went through.
pub const UNFOLLOWED: &str = "You no longer follow this user.";

/// A change went through: added to `playlist`.
#[must_use]
pub fn added_to(playlist: &str) -> String {
    format!("Added to \u{201c}{playlist}\u{201d}.")
}

/// A change went through: removed from `playlist`.
#[must_use]
pub fn removed_from(playlist: &str) -> String {
    format!("Removed from \u{201c}{playlist}\u{201d}.")
}

/// The track was in `playlist` already; nothing was sent.
#[must_use]
pub fn already_in(playlist: &str) -> String {
    format!("This track is already in \u{201c}{playlist}\u{201d}.")
}

/// The track was not in `playlist`; nothing was sent.
#[must_use]
pub fn not_in(playlist: &str) -> String {
    format!("This track is not in \u{201c}{playlist}\u{201d}.")
}

/// A playlist was created.
#[must_use]
pub fn created(title: &str) -> String {
    format!("Created \u{201c}{title}\u{201d} on SoundCloud. It is private.")
}

/// A playlist was renamed.
#[must_use]
pub fn renamed(title: &str) -> String {
    format!("Renamed to \u{201c}{title}\u{201d}.")
}

/// One sentence for a change that did not happen: what was tried, then why.
#[must_use]
pub fn write_failed(action: &str, error: WriteError) -> String {
    let why = match error {
        WriteError::NotSignedIn => "Sign in to SoundCloud first.",
        WriteError::SessionEnded => "SoundCloud ended your sign-in. Sign in again in Settings.",
        WriteError::Refused => "SoundCloud refused the change.",
        WriteError::NotFound => "SoundCloud no longer has it.",
        WriteError::RateLimited => "SoundCloud asked for a pause. Try again in a minute.",
        WriteError::Network => {
            "The request did not finish, so the change may not have been made. Look at SoundCloud, then try again."
        }
        WriteError::Off => "SoundCloud is switched off.",
        WriteError::Invalid => "SoundCloud cannot take that name or item.",
        WriteError::Unchanged => "There was nothing to change.",
    };
    format!("Could not {action}. {why}")
}

/// What sign-out removed, in one sentence.
#[must_use]
pub fn sign_out_result(report: &SignOutReport) -> String {
    let failed = if report.failed_steps.is_empty() {
        String::new()
    } else {
        " One step could not finish; sign out again to retry it.".to_owned()
    };
    format!(
        "Signed out of SoundCloud. Removed {} saved sign-in, {} cache file{} and {} item{} from this computer.{failed}",
        report.credentials_removed,
        report.files_removed,
        if report.files_removed == 1 { "" } else { "s" },
        report.items_removed,
        if report.items_removed == 1 { "" } else { "s" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_copy_is_honest_and_never_claims_more() {
        for text in [HONEST, SETUP_BODY] {
            let lower = text.to_lowercase();
            assert!(lower.contains("unofficial"), "{text}");
            assert!(
                lower.contains("lossy") && lower.contains("not counted"),
                "{text}"
            );
            assert!(
                !lower.contains("lossless") && !lower.contains("safe"),
                "{text}"
            );
        }
        assert!(LOCAL.contains("never sees your password"));
        assert!(LOCAL.contains("only to SoundCloud"));
        assert!(PASTE_HELP.contains("share it with no one"));
        assert!(SIGN_OUT_CONFIRM.contains("does not sign you out of soundcloud.com"));
    }

    #[test]
    fn errors_are_plain_and_hold_no_detail_text() {
        for e in [
            SourceAccountError::Cancelled,
            SourceAccountError::Rejected,
            SourceAccountError::Unavailable("secret-detail-1".to_owned()),
            SourceAccountError::Storage("secret-detail-2".to_owned()),
            SourceAccountError::Network("secret-detail-3".to_owned()),
        ] {
            let text = error(&e);
            assert!(!text.contains("secret-detail"), "{text}");
            assert!(text.ends_with('.'), "{text}");
        }
        assert!(error(&SourceAccountError::Unavailable(String::new())).contains("Paste a token"));
    }

    #[test]
    fn sign_out_names_the_counts_and_a_failed_step() {
        let report = SignOutReport {
            credentials_removed: 1,
            files_removed: 2,
            items_removed: 1,
            failed_steps: vec!["soundcloud-caches"],
            ..SignOutReport::default()
        };
        let text = sign_out_result(&report);
        assert!(
            text.contains("1 saved sign-in, 2 cache files and 1 item"),
            "{text}"
        );
        assert!(text.contains("sign out again"), "{text}");
        assert!(!text.contains("soundcloud-caches"));
    }
}
