//! The settings entry of the SoundCloud switch (ADR 0004, "Opt-in settings pattern").

use nemo_core::opt_in::OptInInfo;

use crate::hosts::HOSTS;

/// What the SoundCloud switch sends, to which hosts, and its risk. Off by default.
///
/// Version 2 (soundcloud-full, ADR 0005): the first notice (version 1) described `yt-dlp` and no
/// account, so everyone who confirmed it confirms this one. The risk text is the ADR's draft; the
/// owner approves its wording (requirements, Open Question 4) before the settings entry ships.
pub const OPT_IN: OptInInfo = OptInInfo {
    id: "soundcloud",
    title: "SoundCloud",
    sends: "Your search text, the SoundCloud pages and tracks you open or play, and, when you \
            sign in, requests for your own SoundCloud account.",
    hosts: HOSTS,
    risk: "SoundCloud is not an official integration. NemoMusic uses the same web calls as \
           soundcloud.com, without an official API key. SoundCloud may block this, may restrict \
           your account if you sign in, and its terms may not allow it. When you sign in, a token \
           for your SoundCloud session is kept in your computer's credential store and is sent \
           only to SoundCloud. Plays here may not count in artists' SoundCloud statistics. \
           SoundCloud audio is lossy (AAC or MP3). Off by default.",
    version: 2,
    default_on: false,
};

#[cfg(test)]
mod tests {
    use super::OPT_IN;
    use crate::hosts::HOSTS;

    #[test]
    fn soundcloud_is_off_by_default_at_version_two() {
        const { assert!(!OPT_IN.default_on) };
        assert_eq!(OPT_IN.version, 2);
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
        assert!(OPT_IN.risk.contains("restrict your account"));
        assert!(OPT_IN.risk.contains("lossy"));
        assert!(!OPT_IN.risk.to_lowercase().contains("safe"));
        assert!(!OPT_IN.risk.to_lowercase().contains("lossless"));
        assert!(OPT_IN.sends.contains("search text"));
    }
}
