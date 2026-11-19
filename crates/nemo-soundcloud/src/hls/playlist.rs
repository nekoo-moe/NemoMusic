//! Reading an on-demand HLS media playlist (RFC 8216) by hand: the few tags SoundCloud uses.
//!
//! The format is small (`#EXT-X-MAP` for the init segment, `#EXTINF` before each segment
//! address, `#EXT-X-ENDLIST` at the end), so no playlist crate is added (tech.md, SoundCloud
//! row). A playlist that names a key (`#EXT-X-KEY` other than `METHOD=NONE`) is refused before
//! any segment is fetched: the app never reads an encrypted stream (`coc.md`).

use std::time::Duration;

/// Most segments one playlist may list. A two-hour track has about 720 ten-second segments.
pub const MAX_SEGMENTS: usize = 4000;

/// One media segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Absolute address. It is signed: never log it.
    pub url: String,
    /// Length of the audio in the segment.
    pub duration: Duration,
}

/// A parsed on-demand playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPlaylist {
    /// The init segment (`#EXT-X-MAP`), absolute.
    pub init: Option<String>,
    /// The segments in order.
    pub segments: Vec<Segment>,
}

impl MediaPlaylist {
    /// Total length of the audio, from the `#EXTINF` values.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.segments.iter().map(|s| s.duration).sum()
    }
}

/// Why a playlist cannot be played. No variant holds an address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlaylistError {
    /// The playlist names an encryption key.
    #[error("the playlist is encrypted")]
    Encrypted,
    /// A master or live playlist, or text that is not a media playlist.
    #[error("not an on-demand media playlist")]
    Unsupported,
    /// A tag or address could not be read, or an address is not allowed.
    #[error("the playlist is malformed")]
    Malformed,
    /// The playlist lists no segment.
    #[error("the playlist lists no segment")]
    Empty,
}

/// Parse `text`. `resolve` turns an address as written (absolute or relative) into an allowed
/// absolute one, or `None` to refuse it.
///
/// # Errors
/// [`PlaylistError`] for a key, a master or live playlist, a bad line or an address `resolve`
/// refuses.
pub fn parse(
    text: &str,
    resolve: &dyn Fn(&str) -> Option<String>,
) -> Result<MediaPlaylist, PlaylistError> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next() != Some("#EXTM3U") {
        return Err(PlaylistError::Unsupported);
    }
    let mut init = None;
    let mut segments = Vec::new();
    let mut pending: Option<Duration> = None;
    let mut ended = false;
    for line in lines {
        if let Some(rest) = line.strip_prefix("#EXT-X-KEY:") {
            if !rest.split(',').any(|a| a.trim() == "METHOD=NONE") {
                return Err(PlaylistError::Encrypted);
            }
        } else if line.starts_with("#EXT-X-STREAM-INF") || line.starts_with("#EXT-X-I-FRAME") {
            return Err(PlaylistError::Unsupported);
        } else if let Some(rest) = line.strip_prefix("#EXT-X-MAP:") {
            let uri = attribute(rest, "URI").ok_or(PlaylistError::Malformed)?;
            if init.is_some() {
                return Err(PlaylistError::Unsupported);
            }
            init = Some(resolve(uri).ok_or(PlaylistError::Malformed)?);
        } else if let Some(rest) = line.strip_prefix("#EXTINF:") {
            pending = Some(extinf(rest)?);
        } else if line == "#EXT-X-ENDLIST" {
            ended = true;
        } else if !line.starts_with('#') {
            let duration = pending.take().ok_or(PlaylistError::Malformed)?;
            if segments.len() >= MAX_SEGMENTS {
                return Err(PlaylistError::Unsupported);
            }
            let url = resolve(line).ok_or(PlaylistError::Malformed)?;
            segments.push(Segment { url, duration });
        }
    }
    if !ended {
        return Err(PlaylistError::Unsupported);
    }
    if segments.is_empty() {
        return Err(PlaylistError::Empty);
    }
    Ok(MediaPlaylist { init, segments })
}

/// `13.5,title` to a duration. A negative, missing or absurd value is malformed.
fn extinf(rest: &str) -> Result<Duration, PlaylistError> {
    let seconds: f64 = rest
        .split(',')
        .next()
        .and_then(|v| v.trim().parse().ok())
        .ok_or(PlaylistError::Malformed)?;
    Duration::try_from_secs_f64(seconds)
        .ok()
        .filter(|d| *d <= Duration::from_hours(1))
        .ok_or(PlaylistError::Malformed)
}

/// The quoted value of `name` in an attribute list such as `URI="a.mp4",BYTERANGE="1@2"`.
fn attribute<'a>(list: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = list.find(&key)? + key.len();
    list[start..].split('"').next()
}

#[cfg(test)]
mod tests;
