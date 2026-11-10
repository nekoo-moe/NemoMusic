//! Fixed wording from `design.md` section 12, ADR 0001 (first-run notice) and the README.
//! Kept in one place so the tests can check it word for word.

/// First-run notice, ADR 0001 (draft approved by the owner for this build), in the paragraphs
/// the sheet shows. Joined with a space they are the ADR text.
pub const TOS_NOTICE: [&str; 3] = [
    "NemoMusic is not made, approved or supported by Google or YouTube. It uses YouTube Music through unofficial means, which may break the YouTube Terms of Service. Google could limit or suspend the account you sign in with, and access can stop working without notice. Use at your own risk, and consider an account you can afford to lose.",
    "NemoMusic runs only on your computer and has no servers: your sign-in is kept in Windows Credential Manager and your library is stored on this PC. Signing out deletes both.",
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
        "Google sign-in page. Only while you sign in (not built yet)",
    ),
];
/// Where local data lives (README).
pub const PRIVACY_DATA: &str = "Your library, history and caches stay in %LOCALAPPDATA%\\NemoMusic. Sign-in data is kept in Windows Credential Manager.";

/// About: licence line.
pub const ABOUT_LICENSE: &str = "NemoMusic is free software under the GPL-3.0 licence. The bundled font is IBM Plex Sans (SIL Open Font License 1.1); the icons were drawn for this project.";
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

#[cfg(test)]
mod tests {
    use super::*;

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
