//! The settings rows of OS media controls, Last.fm and Discord (ADR 0004, "Opt-in settings
//! pattern"). Only media controls is on by default, because nothing leaves the computer.

use nemo_core::opt_in::OptInInfo;

/// OS media controls: the Windows media overlay and MPRIS on Linux. On by default (criterion 9).
pub const MEDIA_CONTROLS: OptInInfo = OptInInfo {
    id: "media_controls",
    title: "OS media controls",
    sends: "Nothing leaves this computer. The title, artist, album and cover are shown by the \
            system.",
    hosts: &[],
    risk: "Anyone who can see your screen or lock screen sees the song.",
    version: 1,
    default_on: true,
};

/// Last.fm scrobbling. Off by default (criterion 10).
pub const LASTFM: OptInInfo = OptInInfo {
    id: "lastfm",
    title: "Last.fm scrobbling",
    sends: "Song titles, artists, albums and play times are sent to Last.fm.",
    hosts: &["ws.audioscrobbler.com"],
    risk: "They show on your public Last.fm profile unless you set it private there.",
    version: 1,
    default_on: false,
};

/// Discord presence. Off by default (criterion 22). Nothing goes on the internet from this app:
/// the text goes to the Discord app on this computer, which shows it to the user's friends and
/// servers by their Discord privacy settings. Discord's own servers load the cover picture from
/// its address.
pub const DISCORD: OptInInfo = OptInInfo {
    id: "discord",
    title: "Discord presence",
    sends: "The title and artist of the song you play, with its source (YouTube Music, SoundCloud \
            or a local file), the address of its cover and the position in the song, and whether \
            you are paused, idle, browsing or away, to the Discord app on this computer. File \
            names and paths are never sent.",
    hosts: &[],
    risk: "They are shown on your Discord profile, and your Discord friends and server members \
           can see them, as your Discord privacy settings allow. Discord's own servers load the \
           cover from its address. The switch under this one hides the song names.",
    version: 1,
    default_on: false,
};

#[cfg(test)]
mod tests {
    use super::{DISCORD, LASTFM, MEDIA_CONTROLS};

    #[test]
    fn only_media_controls_is_on_by_default() {
        let on: Vec<&str> = [MEDIA_CONTROLS, LASTFM, DISCORD]
            .iter()
            .filter(|i| i.default_on)
            .map(|i| i.id)
            .collect();
        assert_eq!(on, ["media_controls"]);
        assert!(MEDIA_CONTROLS.hosts.is_empty() && DISCORD.hosts.is_empty());
    }

    #[test]
    fn lastfm_text_says_what_is_sent_and_where_it_shows() {
        // Criterion 10.
        for part in ["titles", "artists", "albums", "play times", "Last.fm"] {
            assert!(LASTFM.sends.contains(part), "{part}");
        }
        assert!(LASTFM.risk.contains("public Last.fm profile"));
        assert!(LASTFM.risk.contains("private"));
        assert_eq!(LASTFM.hosts, ["ws.audioscrobbler.com"]);
    }

    #[test]
    fn discord_text_says_who_can_see_it() {
        // Criterion 22.
        assert!(DISCORD.sends.contains("title and artist"));
        assert!(DISCORD.risk.contains("Discord profile"));
        assert!(DISCORD.risk.contains("friends and server members"));
        assert!(DISCORD.risk.contains("privacy settings"));
    }

    #[test]
    fn discord_text_names_what_the_new_states_send() {
        // discord-presence criterion 20: source, cover address, position, state, no paths.
        for part in [
            "source", "cover", "position", "paused", "idle", "browsing", "away",
        ] {
            assert!(DISCORD.sends.contains(part), "{part}");
        }
        assert!(DISCORD.sends.contains("never sent"));
        assert!(DISCORD.hosts.is_empty());
    }

    #[test]
    fn no_text_promises_safety() {
        for info in [MEDIA_CONTROLS, LASTFM, DISCORD] {
            let all = format!("{} {}", info.sends, info.risk).to_lowercase();
            assert!(!all.contains("safe"), "{}", info.id);
        }
    }
}
