//! The hosts the SoundCloud feature reaches (soundcloud-full, ADR 0005). Each one is on the
//! README egress list, marked "only when SoundCloud is on".
//!
//! Probed on 2026-11-19: the home page and sign-in page, the web API, the script that holds the
//! client id, covers and avatars, the HLS audio, and the CDN hosts of previews and older MP3
//! files (`cf-preview-media.sndcdn.com` was seen). A host seen in an answer but never contacted
//! (`api.soundcloud.com`, `wave.sndcdn.com`) is not listed.

/// Host names, exact or as `*.suffix` for a family of CDN hosts. No scheme, no path.
pub const HOSTS: &[&str] = &[
    "soundcloud.com",
    "api-v2.soundcloud.com",
    "a-v2.sndcdn.com",
    "i1.sndcdn.com",
    "playback.media-streaming.soundcloud.cloud",
    "*.sndcdn.com",
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
