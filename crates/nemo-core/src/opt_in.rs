//! The one data type behind every opt-in settings row (ADR 0004, "Opt-in settings pattern").
//!
//! A feature that sends data off this computer, reaches a new host, or records what the user
//! plays describes itself with an [`OptInInfo`]. Services return it; the UI draws every row from
//! it and never knows a service by name. No I/O here.

/// What an opt-in feature sends, where to, and its risk, shown before the first request.
///
/// The rules every value follows (ADR 0004, "Opt-in settings pattern"):
/// - **Off by default.** `default_on` is `false`: nothing is requested, built or started while
///   the switch is off. The only exception is OS media controls, which send nothing off the
///   computer.
/// - **Shown before the first request.** The app builds the service only when the switch is on
///   and the stored confirmed version equals [`version`](Self::version). A new host or new data
///   raises `version`, so the user confirms again before the next request.
/// - **The risk text names the real risk.** Unofficial access: "may breach the service's terms
///   and the account can be restricted". Public data: where it shows (a Last.fm profile, a
///   Discord profile). Text sent to a third party: whose terms apply. Local screen text: who can
///   read it. Never "safe", never a promise about the other side.
/// - **Every host is listed.** Each [`hosts`](Self::hosts) entry also appears in the README egress
///   list with `only when <feature> is on`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptInInfo {
    /// Stable id the app stores the switch and the confirmed version under, for example
    /// `"lastfm"`.
    pub id: &'static str,
    /// Row title, for example "Last.fm scrobbling".
    pub title: &'static str,
    /// Plain text: what leaves the computer.
    pub sends: &'static str,
    /// Exact host names, or "the endpoint you set" and "the server you type" when the user
    /// supplies the host. Empty when nothing leaves the computer.
    pub hosts: &'static [&'static str],
    /// Plain text: the real risk of turning the feature on.
    pub risk: &'static str,
    /// Raised whenever a host or the data sent changes, so the confirmation is asked again.
    pub version: u32,
    /// Whether the switch is on in a new install. `false` for every feature except OS media
    /// controls.
    pub default_on: bool,
}

#[cfg(test)]
mod tests {
    use super::OptInInfo;

    const MEDIA_CONTROLS: OptInInfo = OptInInfo {
        id: "media_controls",
        title: "OS media controls",
        sends: "Nothing leaves this computer. The title, artist, album and cover are shown by the system.",
        hosts: &[],
        risk: "Anyone who can see your screen or lock screen sees the song.",
        version: 1,
        default_on: true,
    };

    fn opt_in_features() -> [OptInInfo; 3] {
        [
            OptInInfo {
                id: "lastfm",
                title: "Last.fm scrobbling",
                sends: "The songs you play, with the time you played them.",
                hosts: &["ws.audioscrobbler.com"],
                risk: "Your plays show on your public Last.fm profile.",
                version: 1,
                default_on: false,
            },
            OptInInfo {
                id: "discord",
                title: "Discord presence",
                sends: "The title and artist, to the Discord app on this computer.",
                hosts: &[],
                risk: "Your friends on Discord see what you play.",
                version: 1,
                default_on: false,
            },
            OptInInfo {
                id: "ai",
                title: "AI provider",
                sends: "Seed songs and the messages you see before sending.",
                hosts: &["the endpoint you set"],
                risk: "The provider's terms apply to what you send.",
                version: 1,
                default_on: false,
            },
        ]
    }

    #[test]
    fn only_media_controls_is_on_by_default() {
        let mut all = opt_in_features().to_vec();
        all.push(MEDIA_CONTROLS);
        let on: Vec<&str> = all.iter().filter(|i| i.default_on).map(|i| i.id).collect();
        assert_eq!(on, ["media_controls"]);
        assert!(
            MEDIA_CONTROLS.hosts.is_empty(),
            "the default-on row sends nothing off"
        );
    }
}
