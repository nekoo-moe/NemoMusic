//! Hand-run check against the real SoundCloud web API (soundcloud-full, task 3). Ignored in
//! `cargo test`: it contacts `soundcloud.com`, `a-v2.sndcdn.com` and `api-v2.soundcloud.com`.
//!
//! Run it with `cargo test -p nemo-soundcloud --test live -- --ignored --nocapture`. It sends
//! about 13 requests, one at a time and 250 ms or more apart, and prints counts only: no
//! address, id or title.

use std::sync::Arc;

use nemo_core::catalog::{BrowseId, Catalog, SearchKind};
use nemo_soundcloud::{ClientError, ReqwestHttp, ScClient, SoundcloudCatalog};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "contacts the real SoundCloud; run by hand"]
async fn search_pages_and_related_work_on_the_real_service() {
    let http = ReqwestHttp::new(tokio::runtime::Handle::current()).expect("client");
    let client = Arc::new(ScClient::new(Arc::new(http)));
    let began = std::time::Instant::now();
    match client.warm_up().await {
        Ok(()) => {}
        Err(ClientError::RateLimited) => {
            println!("rate limited: this IP is being slowed down, nothing more to check");
            return;
        }
        Err(e) => panic!("client id lookup failed: {e}"),
    }
    println!("client id: {} ms", began.elapsed().as_millis());
    let catalog = SoundcloudCatalog::new(client);

    let results = catalog.search("ambient piano").await.expect("search");
    println!(
        "search: {} songs, {} albums, {} artists, {} playlists",
        results.songs.len(),
        results.albums.len(),
        results.artists.len(),
        results.playlists.len()
    );
    assert!(!results.songs.is_empty());
    assert!(
        results
            .songs
            .iter()
            .all(|t| t.source_url.is_some() && !t.artists.is_empty())
    );

    let songs = catalog
        .search_kind("ambient piano", SearchKind::Songs)
        .await
        .expect("songs");
    println!("songs filter: {}", songs.songs.len());

    let related = catalog
        .related(&results.songs[0].id)
        .await
        .expect("related");
    println!("related: {}", related.len());

    if let Some(list) = results.playlists.iter().find(|p| !p.title.is_empty()) {
        let page = catalog.playlist(&list.id).await.expect("playlist page");
        println!("playlist page: {} tracks", page.tracks.len());
    }
    if let Some(artist) = results.artists.first() {
        let id = BrowseId(artist.id.0.clone());
        let page = catalog.artist(&id).await.expect("artist page");
        println!(
            "artist page: {} songs, {} albums",
            page.top_songs.len(),
            page.albums.len()
        );
    }
    println!("total: {} ms", began.elapsed().as_millis());
}
