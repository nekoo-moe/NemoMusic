//! JSON builders shared by the feed tests.

use serde_json::{Value, json};

pub(super) const ID: &str = "abcdefghijk";
pub(super) const ID2: &str = "lmnopqrstuv";

pub(super) fn runs(texts: &[&str]) -> Value {
    json!({ "runs": texts.iter().map(|t| json!({ "text": t })).collect::<Vec<_>>() })
}

/// A run that links to a page.
pub(super) fn link(text: &str, browse_id: &str) -> Value {
    json!({ "text": text, "navigationEndpoint": { "browseEndpoint": { "browseId": browse_id } } })
}

pub(super) fn thumbs(url: &str) -> Value {
    json!({ "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
        { "url": url, "width": 120, "height": 120 }
    ] } } })
}

pub(super) fn browse(id: &str, page_type: &str) -> Value {
    json!({ "browseEndpoint": {
        "browseId": id,
        "browseEndpointContextSupportedConfigs": { "browseEndpointContextMusicConfig": {
            "pageType": page_type
        } }
    } })
}

pub(super) fn card(title: &str, subtitle: Value, nav: Value) -> Value {
    json!({ "musicTwoRowItemRenderer": {
        "title": runs(&[title]),
        "subtitle": subtitle,
        "thumbnailRenderer": thumbs("https://img.invalid/cover"),
        "thumbnailCornerOverlay": thumbs("https://img.invalid/avatar"),
        "navigationEndpoint": nav
    } })
}

pub(super) fn watch(video_id: &str, kind: &str) -> Value {
    json!({ "watchEndpoint": {
        "videoId": video_id,
        "watchEndpointMusicSupportedConfigs": { "watchEndpointMusicConfig": {
            "musicVideoType": kind
        } }
    } })
}

pub(super) fn row(
    title: &str,
    video_id: &str,
    artist: &str,
    album: Option<&str>,
    duration: &str,
) -> Value {
    let mut columns = vec![
        json!({ "musicResponsiveListItemFlexColumnRenderer": { "text": runs(&[title]) } }),
        json!({ "musicResponsiveListItemFlexColumnRenderer": {
            "text": { "runs": [ link(artist, "UCsynthetic1") ] } } }),
    ];
    if let Some(album) = album {
        columns.push(json!({ "musicResponsiveListItemFlexColumnRenderer": {
            "text": { "runs": [ link(album, "MPREbsynthetic1") ] } } }));
    }
    json!({ "musicResponsiveListItemRenderer": {
        "thumbnail": thumbs("https://img.invalid/row"),
        "flexColumns": columns,
        "fixedColumns": [ { "musicResponsiveListItemFixedColumnRenderer": {
            "text": runs(&[duration]) } } ],
        "playlistItemData": { "videoId": video_id }
    } })
}

pub(super) fn carousel(title: &str, strapline: Option<&str>, items: Vec<Value>) -> Value {
    let mut header = json!({ "title": runs(&[title]) });
    if let Some(line) = strapline {
        header["strapline"] = runs(&[line]);
    }
    json!({ "musicCarouselShelfRenderer": {
        "header": { "musicCarouselShelfBasicHeaderRenderer": header },
        "contents": items
    } })
}

pub(super) fn section_list(sections: Vec<Value>) -> Value {
    json!({ "contents": { "singleColumnBrowseResultsRenderer": { "tabs": [ {
        "tabRenderer": { "content": { "sectionListRenderer": { "contents": sections } } }
    } ] } } })
}

pub(super) fn chips() -> Value {
    json!({ "chipCloudRenderer": { "chips": [
        { "chipCloudChipRenderer": { "text": runs(&["Relax"]), "navigationEndpoint": {
            "browseEndpoint": { "browseId": "FEmusic_home", "params": "ggM8abc%3D" } } } },
        { "chipCloudChipRenderer": { "text": runs(&["No params"]), "navigationEndpoint": {
            "browseEndpoint": { "browseId": "FEmusic_home" } } } }
    ] } })
}
