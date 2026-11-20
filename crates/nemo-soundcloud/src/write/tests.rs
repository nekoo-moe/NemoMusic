//! The write requests: methods, paths and bodies, with invented ids.

use serde_json::json;

use super::*;

#[test]
fn likes_are_put_and_delete_on_the_users_own_path() {
    let like = WriteCall::track_like(7_000_001, 5_000_002, true);
    assert_eq!(
        (like.method, like.path.as_str(), &like.body),
        (ScMethod::Put, "/users/7000001/track_likes/5000002", &None)
    );
    let unlike = WriteCall::track_like(7_000_001, 5_000_002, false);
    assert_eq!(unlike.method, ScMethod::Delete);
    assert_eq!(unlike.path, like.path);
    let playlist = WriteCall::playlist_like(7_000_001, 6_000_003, true);
    assert_eq!(playlist.path, "/users/7000001/playlist_likes/6000003");
    assert_eq!(
        WriteCall::playlist_like(7_000_001, 6_000_003, false).method,
        ScMethod::Delete
    );
}

#[test]
fn unfollow_is_a_delete_and_there_is_no_follow() {
    let call = WriteCall::unfollow(8_000_004);
    assert_eq!(
        (call.method, call.path.as_str()),
        (ScMethod::Delete, "/me/followings/8000004")
    );
    assert!(call.body.is_none());
}

#[test]
fn a_new_playlist_is_private_and_wrapped_like_the_web_apps_save() {
    let call = WriteCall::create_playlist("Night drive", &[5_000_002]);
    assert_eq!(
        (call.method, call.path.as_str()),
        (ScMethod::Post, "/playlists")
    );
    assert_eq!(
        call.body,
        Some(json!({"playlist": {
            "title": "Night drive",
            "sharing": "private",
            "tracks": [5_000_002_u64],
            "_resource_type": "playlist",
        }}))
    );
}

#[test]
fn track_changes_send_the_whole_list_and_a_rename_only_the_title() {
    let tracks = WriteCall::set_tracks(6_000_003, &[1, 2, 3]);
    assert_eq!(
        (tracks.method, tracks.path.as_str()),
        (ScMethod::Put, "/playlists/6000003")
    );
    assert_eq!(
        tracks.body,
        Some(json!({"playlist": {"tracks": [1, 2, 3]}}))
    );
    let rename = WriteCall::rename(6_000_003, "New name");
    assert_eq!(
        rename.body,
        Some(json!({"playlist": {"title": "New name"}}))
    );
}

#[test]
fn a_title_is_escaped_by_json_and_cannot_change_the_path() {
    let call = WriteCall::create_playlist("a \"quote\" and \\ and /../me", &[]);
    let text = serde_json::to_string(&call.body).expect("json");
    assert!(text.contains(r#"a \"quote\" and \\ and /../me"#), "{text}");
    assert_eq!(call.path, "/playlists");
}

#[test]
fn titles_are_cleaned_and_limited() {
    assert_eq!(
        clean_title("  Night drive \n").as_deref(),
        Some("Night drive")
    );
    assert_eq!(clean_title("tab\there").as_deref(), Some("tabhere"));
    assert_eq!(clean_title("   "), None);
    assert_eq!(clean_title("\u{7}"), None);
    assert!(clean_title(&"a".repeat(MAX_TITLE_CHARS)).is_some());
    assert_eq!(clean_title(&"a".repeat(MAX_TITLE_CHARS + 1)), None);
    // Characters, not bytes.
    assert!(clean_title(&"é".repeat(MAX_TITLE_CHARS)).is_some());
}

#[test]
fn track_ids_include_tracks_the_answer_holds_only_as_an_id() {
    let answer = json!({"tracks": [{"id": 1, "title": "x"}, {"id": 2}, {"id": 3}]});
    assert_eq!(track_ids(&answer), Some(vec![1, 2, 3]));
    assert_eq!(track_ids(&json!({"tracks": []})), Some(Vec::new()));
    assert_eq!(track_ids(&json!({"tracks": [{"title": "no id"}]})), None);
    assert_eq!(track_ids(&json!({})), None);
    assert_eq!(created_id(&json!({"id": 9_000_005_u64})), Some(9_000_005));
    assert_eq!(created_id(&json!({"id": "text"})), None);
}
