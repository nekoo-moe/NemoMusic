//! The hosts the SoundCloud feature reaches, through `yt-dlp` and the stream reader.
//!
//! These are the four hosts of the design. The network capture of the spike (soundcloud task 7)
//! replaces them, and the owner approves that list before the feature ships. Each one is also on
//! the README egress list, marked "only when SoundCloud is on", from the task that first
//! contacts it.

/// Host names, exact or as `*.suffix` for a family of CDN hosts. No scheme, no path.
pub const HOSTS: &[&str] = &[
    "soundcloud.com",
    "api-v2.soundcloud.com",
    "*.sndcdn.com",
    "playback.media-streaming.soundcloud.cloud",
];

#[cfg(test)]
mod tests {
    use super::HOSTS;

    #[test]
    fn hosts_are_bare_lowercase_names() {
        for host in HOSTS {
            let name = host.strip_prefix("*.").unwrap_or(host);
            assert!(!name.is_empty() && name.contains('.'), "{host}");
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-.".contains(c)),
                "{host}: no scheme, port, path or upper case"
            );
            assert!(
                !name.contains('*'),
                "{host}: a wildcard only as a `*.` prefix"
            );
        }
    }

    #[test]
    fn hosts_are_listed_once() {
        let mut sorted = HOSTS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), HOSTS.len());
    }
}
