//! The settings entry of the SoundCloud switch (ADR 0004, "Opt-in settings pattern").

use nemo_core::opt_in::OptInInfo;

use crate::hosts::HOSTS;

/// What the SoundCloud switch sends, to which hosts, and its risk. Off by default.
///
/// The risk text is the design's draft; the owner approves its wording (requirements, Open
/// Question 1) before the settings entry ships.
pub const OPT_IN: OptInInfo = OptInInfo {
    id: "soundcloud",
    title: "SoundCloud",
    sends: "Your search text and the SoundCloud page addresses you open, through yt-dlp.",
    hosts: HOSTS,
    risk: "SoundCloud is not an official integration. NemoMusic reads public pages the way a \
           browser does, through yt-dlp. SoundCloud may block this, and its terms may not allow \
           it. Off by default.",
    version: 1,
    default_on: false,
};

#[cfg(test)]
mod tests {
    use super::OPT_IN;
    use crate::hosts::HOSTS;

    #[test]
    fn soundcloud_is_off_by_default_at_version_one() {
        const { assert!(!OPT_IN.default_on) };
        assert_eq!(OPT_IN.version, 1);
        assert_eq!(OPT_IN.id, "soundcloud");
    }

    #[test]
    fn the_entry_shows_every_host() {
        assert_eq!(OPT_IN.hosts, HOSTS);
    }

    #[test]
    fn the_risk_text_names_the_real_risk() {
        assert!(OPT_IN.risk.contains("not an official integration"));
        assert!(OPT_IN.risk.contains("terms"));
        assert!(!OPT_IN.risk.to_lowercase().contains("safe"));
        assert!(OPT_IN.sends.contains("yt-dlp"));
    }
}
