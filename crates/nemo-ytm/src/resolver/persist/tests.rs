//! Tests of the file of kept streams. Temporary folders, no network.

use super::*;

fn stream(url: &str, expires_in: Duration) -> ResolvedStream {
    ResolvedStream {
        url: Secret::new(url.to_owned()),
        info: StreamInfo {
            origin: Origin::YoutubeMusic,
            codec: Codec::Opus,
            bitrate_kbps: Some(130),
            format: Some(AudioFormat {
                sample_rate: 48_000,
                channels: 2,
                bit_depth: None,
            }),
        },
        expires_at: Some(SystemTime::now() + expires_in),
        transport: Transport::File,
        preview: false,
        audio_language: None,
        audio_tracks: Vec::new(),
    }
}

fn vid(text: &str) -> VideoId {
    VideoId::parse(text).expect("valid")
}

fn folder(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-streams-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // A leftover of an earlier run is fine to drop.
    dir
}

#[test]
fn a_kept_stream_comes_back_after_a_restart_with_what_it_had() {
    let dir = folder("roundtrip");
    let path = dir.join("cache").join("streams.json");
    let store = StreamStore::open(path.clone());
    let video = vid("abcdefghijk");
    let fresh = stream("https://a.invalid/one", Duration::from_hours(5));
    assert!(store.put(&video, &fresh));
    store.flush();

    let again = StreamStore::open(path).get(&video).expect("kept");
    assert_eq!(again.url.expose(), "https://a.invalid/one");
    assert_eq!(again.info, fresh.info);
    assert_eq!(again.transport, Transport::File);
    assert_eq!(
        secs(again.expires_at.expect("expiry")),
        secs(fresh.expires_at.expect("expiry"))
    );
    let _ = std::fs::remove_dir_all(&dir); // Cleanup only.
}

#[test]
fn the_language_and_the_audio_tracks_of_a_stream_are_kept_with_it() {
    let dir = folder("tracks");
    let path = dir.join("streams.json");
    let mut dubbed = stream("https://a.invalid/x", Duration::from_hours(5));
    dubbed.audio_language = Some("en".to_owned());
    dubbed.audio_tracks = vec![AudioTrackInfo {
        language: Some("en".to_owned()),
        label: "English (original)".to_owned(),
        is_default: true,
    }];
    let store = StreamStore::open(path.clone());
    assert!(store.put(&vid("abcdefghijk"), &dubbed));
    store.flush();
    let back = StreamStore::open(path)
        .get(&vid("abcdefghijk"))
        .expect("kept");
    assert_eq!(back.audio_language, dubbed.audio_language);
    assert_eq!(back.audio_tracks, dubbed.audio_tracks);
    let _ = std::fs::remove_dir_all(&dir); // Cleanup only.
}

#[test]
fn only_a_plain_stream_valid_for_longer_than_the_margin_is_kept() {
    let store = StreamStore::open(folder("plain").join("s.json"));
    let video = vid("abcdefghijk");
    let soon = stream("https://a.invalid/x", Duration::from_mins(5));
    let mut preview = stream("https://a.invalid/x", Duration::from_hours(5));
    preview.preview = true;
    let mut hls = stream("https://a.invalid/x", Duration::from_hours(5));
    hls.transport = Transport::Hls { live: false };
    let mut no_expiry = stream("https://a.invalid/x", Duration::from_hours(5));
    no_expiry.expires_at = None;
    for refused in [soon, preview, hls, no_expiry] {
        assert!(!store.put(&video, &refused));
    }
    assert!(store.is_empty());
}

#[test]
fn an_expired_entry_is_not_loaded_and_a_damaged_file_is_no_file() {
    let dir = folder("expired");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("streams.json");
    let old = r#"{"version":1,"streams":{"abcdefghijk":{"url":"https://a.invalid/x","expires":1000,"codec":"aac","kbps":null,"rate":null,"channels":null,"depth":null}}}"#;
    for text in [old, "not json", r#"{"version":9,"streams":{}}"#, ""] {
        std::fs::write(&path, text).expect("write");
        assert!(StreamStore::open(path.clone()).is_empty(), "{text}");
    }
    let _ = std::fs::remove_dir_all(&dir); // Cleanup only.
}

#[cfg(unix)]
#[test]
fn the_file_is_private_to_its_owner() {
    use std::os::unix::fs::PermissionsExt;

    let dir = folder("mode");
    let path = dir.join("streams.json");
    let store = StreamStore::open(path.clone());
    assert!(store.put(
        &vid("abcdefghijk"),
        &stream("https://a.invalid/x", Duration::from_hours(5))
    ));
    store.flush();
    let mode = std::fs::metadata(&path).expect("file").permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    assert!(
        !path.with_extension("json.tmp").exists(),
        "no temporary file is left"
    );
    let _ = std::fs::remove_dir_all(&dir); // Cleanup only.
}

#[test]
fn forget_and_the_sign_out_step_delete_what_was_kept() {
    let dir = folder("wipe");
    let path = dir.join("streams.json");
    let store = StreamStore::open(path.clone());
    let (a, b) = (vid("abcdefghijk"), vid("bbbbbbbbbbb"));
    for video in [&a, &b] {
        assert!(store.put(
            video,
            &stream("https://a.invalid/x", Duration::from_hours(5))
        ));
    }
    store.flush();
    store.forget(&a);
    assert!(store.get(&a).is_none() && store.get(&b).is_some());
    assert!(
        StreamStore::open(path.clone()).get(&a).is_none(),
        "the file was rewritten"
    );

    assert_eq!(SignOutWipe::name(&store), "streams");
    assert!(SignOutWipe::wipe(&store, WipeScope::All).expect("wiped") >= 1);
    assert!(store.is_empty() && !path.exists());
    assert_eq!(SignOutWipe::wipe(&store, WipeScope::All).expect("again"), 0);
    let _ = std::fs::remove_dir_all(&dir); // Cleanup only.
}

#[test]
fn the_store_holds_at_most_as_many_entries_as_the_memory_cache() {
    let store = StreamStore::open(folder("cap").join("s.json"));
    for n in 0..(MAX_CACHED + 5) {
        let id = format!("{n:011}");
        assert!(store.put(
            &vid(&id),
            &stream("https://a.invalid/x", Duration::from_hours(5))
        ));
    }
    assert_eq!(store.len(), MAX_CACHED);
}
