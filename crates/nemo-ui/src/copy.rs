//! Fixed wording from `design.md` section 12, ADR 0001 (first-run notice) and the README.
//! Kept in one place so the tests can check it word for word.
//!
//! The credential store and data folder differ per platform, so the sentences that name them
//! are built with the `store!`, `data_dir!` and `this_machine!` macros. The Windows text is the
//! `design.md` wording.

/// Name of the OS credential store, as the user sees it.
#[cfg(windows)]
macro_rules! store {
    () => {
        "Windows Credential Manager"
    };
}
/// Name of the OS credential store, as the user sees it.
#[cfg(not(windows))]
macro_rules! store {
    () => {
        "the system keyring"
    };
}
/// The app data folder, as the user sees it.
#[cfg(windows)]
macro_rules! data_dir {
    () => {
        "%LOCALAPPDATA%\\NemoMusic"
    };
}
/// The app data folder, as the user sees it.
#[cfg(not(windows))]
macro_rules! data_dir {
    () => {
        "$XDG_DATA_HOME/NemoMusic (usually ~/.local/share/NemoMusic)"
    };
}

/// How the first-run notice names this computer (the ADR text says "this PC" on Windows).
#[cfg(windows)]
macro_rules! this_machine {
    () => {
        "this PC"
    };
}
/// How the first-run notice names this computer.
#[cfg(not(windows))]
macro_rules! this_machine {
    () => {
        "this computer"
    };
}

/// Name of the OS credential store, as the user sees it.
pub const CREDENTIAL_STORE: &str = store!();

/// First-run notice, ADR 0001 (draft approved by the owner for this build), in the paragraphs
/// the sheet shows. Joined with a space they are the ADR text.
pub const TOS_NOTICE: [&str; 3] = [
    "NemoMusic is not made, approved or supported by Google or YouTube. It uses YouTube Music through unofficial means, which may break the YouTube Terms of Service. Google could limit or suspend the account you sign in with, and access can stop working without notice. Use at your own risk, and consider an account you can afford to lose.",
    concat!(
        "NemoMusic runs only on your computer and has no servers: your sign-in is kept in ",
        store!(),
        " and your library is stored on ",
        this_machine!(),
        ". Signing out deletes both."
    ),
    "Ad blocking and SponsorBlock can reduce income for artists; both can be switched off in Settings. YouTube Music audio is lossy.",
];
/// Label of the notice checkbox (not pre-checked).
pub const TOS_CHECKBOX: &str = "I have read this notice";

/// SponsorBlock switch label (`design.md` section 12).
pub const SPONSORBLOCK_LABEL: &str = "Skip non-music parts of music videos";
/// SponsorBlock source line.
pub const SPONSORBLOCK_SOURCE: &str = "Data from SponsorBlock";
/// SponsorBlock licence line (README, ADR 0003; legal reading unverified).
pub const SPONSORBLOCK_LICENSE: &str = "SponsorBlock data by Ajay Ramachandran, CC BY-NC-SA 4.0, fetched live from sponsor.ajay.app. NemoMusic stays non-commercial while it uses that data.";
/// Ad blocking switch label (`design.md` section 12).
pub const ADBLOCK_LABEL: &str = "Don't request or play ads";
/// Ad blocking helper text (`design.md` section 12).
pub const ADBLOCK_HELPER: &str = "This can restrict your account and reduces income for artists. It does not unlock Premium quality.";

/// Intro of the Privacy section.
pub const PRIVACY_INTRO: &str = "NemoMusic runs entirely on your computer. The project runs no server and collects no telemetry. These are the only hosts it contacts:";
/// The network egress list (README), `(host, why and when)`.
pub const EGRESS: [(&str, &str); 7] = [
    (
        "music.youtube.com",
        "Search, browse, timed lyrics. Always (YouTube Music features)",
    ),
    (
        "www.youtube.com",
        "Stream lookup by yt-dlp. When a YouTube track plays",
    ),
    (
        "*.googlevideo.com",
        "The audio stream itself. When a YouTube track plays",
    ),
    (
        "i.ytimg.com, *.ggpht.com, *.googleusercontent.com",
        "Cover art and artist pictures. When covers are shown",
    ),
    ("lrclib.net", "Synced lyrics. When lyrics are shown"),
    (
        "sponsor.ajay.app",
        "SponsorBlock segments (only a 4-character hash prefix is sent). Only when SponsorBlock is on",
    ),
    (
        "accounts.google.com, *.gstatic.com",
        "Google sign-in page. Only while you sign in",
    ),
];
/// Where local data lives (README).
pub const PRIVACY_DATA: &str = concat!(
    "Your library, history and caches stay in ",
    data_dir!(),
    ". Sign-in data is kept in ",
    store!(),
    "."
);

/// About: licence line.
pub const ABOUT_LICENSE: &str = "NemoMusic is free software under the GPL-3.0 licence. The bundled font is Google Sans Flex (SIL Open Font License 1.1); the icons are Material Symbols Rounded (Apache License 2.0).";
/// About: credits.
pub const ABOUT_CREDITS: [&str; 2] = [
    "Lyrics: LRCLIB (lrclib.net) and YouTube Music. The provider name is shown with every lyric.",
    "Skip data: SponsorBlock (sponsor.ajay.app) by Ajay Ramachandran, CC BY-NC-SA 4.0, fetched live.",
];
/// About: terms of service notice.
pub const ABOUT_TOS: &str = "NemoMusic is not made, approved or supported by Google or YouTube. It uses YouTube Music without an official API. This may break the YouTube Terms of Service, and your account could be restricted.";

/// Library empty state.
pub const LIBRARY_EMPTY_TITLE: &str = "Your library is empty";
/// Library empty state, how to add music.
pub const LIBRARY_EMPTY_HELP: &str = "Add a music folder in Settings, under Library. NemoMusic reads FLAC, ALAC, WAV, MP3, AAC and Ogg files and keeps the list on this computer.";

/// Sign-in sheet: title when signed out.
pub const ACCOUNT_TITLE: &str = "Sign in to YouTube Music";
/// Sign-in sheet: where everything lives (`design.md` section 11, word for word).
pub const ACCOUNT_LOCAL: &str = concat!(
    "Everything stays on this computer. NemoMusic has no server. Your sign-in and library data are stored here, in ",
    store!(),
    "."
);
/// Sign-in sheet: the Terms of Service notice (`design.md` section 11, word for word).
pub const ACCOUNT_TOS: &str = "NemoMusic uses YouTube Music without an official API. This may break YouTube's Terms of Service and your account could be restricted.";
/// Sign-in sheet: primary button.
pub const ACCOUNT_SIGN_IN: &str = "Sign in with Google";
/// Sign-in sheet: fallback toggle.
pub const ACCOUNT_PASTE: &str = "Paste cookie instead";
/// Sign-in sheet: back from the fallback.
pub const ACCOUNT_USE_WINDOW: &str = "Use the sign-in window instead";
/// Sign-in sheet: help under the cookie field.
pub const ACCOUNT_COOKIE_HELP: &str = concat!(
    "Copy the cookie header of a signed-in music.youtube.com request from your own browser. It is hidden while you type, kept in ",
    store!(),
    ", and never shown again."
);
/// Sign-in sheet: placeholder of the masked field.
pub const ACCOUNT_COOKIE_PLACEHOLDER: &str = "Paste the cookie here";
/// Sign-in sheet: waiting for the sign-in window.
pub const ACCOUNT_WAITING: &str =
    "Finish signing in in the window that opened. This sheet updates by itself.";
/// Sign-in sheet: checking a pasted cookie.
pub const ACCOUNT_CHECKING: &str = "Checking with YouTube Music\u{2026}";
/// Sign-in sheet: the notice has not been accepted yet.
pub const ACCOUNT_NEEDS_NOTICE: &str = "Read the first-run notice before you sign in.";
/// Sign-out confirmation (`design.md` section 11, word for word).
pub const ACCOUNT_SIGN_OUT_CONFIRM: &str =
    "This removes your sign-in, library cache, history and lyrics cache from this computer.";

/// Plain words for a sign-in or sign-out failure. Never contains the cookie or any token.
#[must_use]
pub fn account_error(error: &nemo_core::account::AccountError) -> String {
    use nemo_core::account::AccountError as E;
    match error {
        E::Cancelled => {
            "The sign-in window was closed before you finished. You are still signed out."
                .to_owned()
        }
        E::Blocked => "Google would not show its sign-in page in this window. Use \u{201c}Paste cookie instead\u{201d} below.".to_owned(),
        E::Rejected => "YouTube Music did not accept this sign-in. If you pasted a cookie, check that you copied all of it from a browser where you are signed in.".to_owned(),
        E::Unavailable(_) => "The sign-in window cannot open on this computer. Use \u{201c}Paste cookie instead\u{201d} below.".to_owned(),
        E::Storage(_) => concat!("NemoMusic could not save the sign-in in ", store!(), ", so you are still signed out.").to_owned(),
        E::Network(_) => "The network request failed. Check your connection and try again.".to_owned(),
    }
}

/// What sign-out removed, in one sentence.
#[must_use]
pub fn sign_out_result(report: &nemo_core::account::SignOutReport) -> String {
    let remote = if report.ended_remotely {
        " The session was also ended on Google\u{2019}s side."
    } else {
        ""
    };
    format!(
        "Signed out. Removed {} saved sign-in{} and {} cache file{} from this computer.{remote}",
        report.credentials_removed,
        if report.credentials_removed == 1 {
            ""
        } else {
            "s"
        },
        report.files_removed,
        if report.files_removed == 1 { "" } else { "s" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_wording_matches_design_section_11() {
        assert!(ACCOUNT_LOCAL.starts_with("Everything stays on this computer."));
        #[cfg(windows)]
        assert_eq!(
            ACCOUNT_LOCAL,
            "Everything stays on this computer. NemoMusic has no server. Your sign-in and library data are stored here, in Windows Credential Manager."
        );
        assert!(ACCOUNT_LOCAL.ends_with(concat!(store!(), ".")));
        assert!(ACCOUNT_TOS.contains("could be restricted"));
        assert_eq!(
            ACCOUNT_SIGN_OUT_CONFIRM,
            "This removes your sign-in, library cache, history and lyrics cache from this computer."
        );
    }

    #[test]
    fn account_errors_are_plain_and_leak_nothing() {
        use nemo_core::account::AccountError as E;
        for e in [
            E::Cancelled,
            E::Blocked,
            E::Rejected,
            E::Unavailable("SECRET-DETAIL".to_owned()),
            E::Storage("SECRET-DETAIL".to_owned()),
            E::Network("SECRET-DETAIL".to_owned()),
        ] {
            let text = account_error(&e);
            assert!(!text.contains("SECRET-DETAIL") && text.ends_with('.'));
        }
    }

    #[test]
    fn sign_out_result_counts() {
        let r = nemo_core::account::SignOutReport {
            credentials_removed: 1,
            files_removed: 3,
            ended_remotely: false,
            ..nemo_core::account::SignOutReport::default()
        };
        assert_eq!(
            sign_out_result(&r),
            "Signed out. Removed 1 saved sign-in and 3 cache files from this computer."
        );
    }

    #[test]
    fn the_notice_is_the_adr_text() {
        let joined = TOS_NOTICE.join(" ");
        assert!(
            joined
                .starts_with("NemoMusic is not made, approved or supported by Google or YouTube.")
        );
        assert!(joined.ends_with("YouTube Music audio is lossy."));
        assert_eq!(joined.matches("Terms of Service").count(), 1);
    }

    #[test]
    fn ad_blocking_wording_matches_design_section_12() {
        assert_eq!(ADBLOCK_LABEL, "Don't request or play ads");
        assert_eq!(
            ADBLOCK_HELPER,
            "This can restrict your account and reduces income for artists. It does not unlock Premium quality."
        );
        assert_eq!(SPONSORBLOCK_LABEL, "Skip non-music parts of music videos");
    }

    #[test]
    fn egress_list_has_the_seven_readme_rows() {
        assert_eq!(EGRESS.len(), 7);
        assert!(EGRESS.iter().any(|(h, _)| *h == "lrclib.net"));
    }
}
