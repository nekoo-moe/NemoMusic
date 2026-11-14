//! Search over all of YouTube: the search scope, video results, channels and live streams
//! (youtube-extras).

use super::{BrowseId, PlaylistSummary};
use crate::track::{Thumbnail, Track};

/// Where a search looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SearchScope {
    /// YouTube Music only.
    #[default]
    Music,
    /// All of YouTube: videos and channels too.
    AllOfYoutube,
}

/// A channel named by a video result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelRef {
    /// Id for [`Catalog::channel`](super::Catalog::channel).
    pub id: BrowseId,
    /// Channel name.
    pub name: String,
}

/// A YouTube video in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoSummary {
    /// The video as a playable track.
    pub track: Track,
    /// The channel that uploaded it, if shown.
    pub channel: Option<ChannelRef>,
    /// The video is a live stream now.
    pub live: bool,
    /// The view count as YouTube shows it, such as "1.2M views".
    pub views: Option<String>,
}

/// A YouTube channel in a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSummary {
    /// Id for [`Catalog::channel`](super::Catalog::channel).
    pub id: BrowseId,
    /// Channel name.
    pub name: String,
    /// The subscriber count as YouTube shows it, if shown.
    pub subscribers: Option<String>,
    /// Picture sizes.
    pub thumbnails: Vec<Thumbnail>,
}

/// A channel page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelDetail {
    /// Header data.
    pub summary: ChannelSummary,
    /// Uploaded videos, in the order the page lists them.
    pub uploads: Vec<VideoSummary>,
    /// The channel's playlists.
    pub playlists: Vec<PlaylistSummary>,
}
