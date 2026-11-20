use std::io::Cursor;
use std::path::PathBuf;

use serde_json::json;

use super::{
    Incoming, MAX_BODY, OP_CLOSE, OP_FRAME, OP_PING, activity_body, candidates, classify, encode,
    handshake_body, read_frame,
};
use crate::discord::{Activity, ActivityKind};

#[test]
fn a_frame_round_trips() {
    let bytes = encode(OP_FRAME, b"{\"a\":1}");
    assert_eq!(&bytes[..8], &[1, 0, 0, 0, 7, 0, 0, 0]);
    let (op, body) = read_frame(&mut Cursor::new(bytes)).expect("frame");
    assert_eq!((op, body.as_slice()), (OP_FRAME, &b"{\"a\":1}"[..]));
}

#[test]
fn a_frame_that_is_too_long_or_cut_short_is_an_error() {
    let mut long = Vec::new();
    long.extend_from_slice(&OP_FRAME.to_le_bytes());
    long.extend_from_slice(&u32::try_from(MAX_BODY + 1).expect("fits").to_le_bytes());
    assert!(read_frame(&mut Cursor::new(long)).is_err());
    let mut cut = encode(OP_FRAME, b"hello");
    cut.truncate(10);
    assert!(read_frame(&mut Cursor::new(cut)).is_err());
    assert!(read_frame(&mut Cursor::new(Vec::new())).is_err());
}

#[test]
fn the_handshake_names_the_application() {
    let body: serde_json::Value = serde_json::from_slice(&handshake_body("123")).expect("json");
    assert_eq!(body, json!({"v": 1, "client_id": "123"}));
}

#[test]
fn set_activity_and_clear_commands_have_the_ipc_shape() {
    let activity = Activity {
        kind: ActivityKind::Playing,
        details: "Idle".into(),
        state: None,
        start: None,
        end: None,
        large: None,
        small: None,
    };
    let set: serde_json::Value =
        serde_json::from_slice(&activity_body(42, 7, Some(&activity))).expect("json");
    assert_eq!(
        set,
        json!({
            "cmd": "SET_ACTIVITY",
            "args": {"pid": 42, "activity": {"type": 0, "details": "Idle"}},
            "nonce": "7"
        })
    );
    let clear: serde_json::Value =
        serde_json::from_slice(&activity_body(42, 8, None)).expect("json");
    assert_eq!(
        clear,
        json!({"cmd": "SET_ACTIVITY", "args": {"pid": 42}, "nonce": "8"})
    );
}

#[test]
fn incoming_frames_are_classified_without_keeping_text() {
    let ready = br#"{"cmd":"DISPATCH","evt":"READY","data":{"user":{"username":"x"}}}"#;
    assert_eq!(classify(OP_FRAME, ready), Incoming::Ready);
    let error = br#"{"cmd":"SET_ACTIVITY","evt":"ERROR","data":{"code":4000}}"#;
    assert_eq!(classify(OP_FRAME, error), Incoming::Error);
    let ok = br#"{"cmd":"SET_ACTIVITY","evt":null,"data":{}}"#;
    assert_eq!(classify(OP_FRAME, ok), Incoming::Other);
    assert_eq!(classify(OP_FRAME, b"not json"), Incoming::Other);
    assert_eq!(classify(OP_PING, b"abc"), Incoming::Ping(b"abc".to_vec()));
    assert_eq!(classify(OP_CLOSE, b"{}"), Incoming::Close);
    assert_eq!(classify(9, b"{}"), Incoming::Other);
}

#[cfg(unix)]
#[test]
fn unix_paths_cover_the_runtime_dir_flatpak_snap_and_temp_dirs() {
    let env = |name: &str| match name {
        "XDG_RUNTIME_DIR" => Some("/run/user/1000".to_owned()),
        "TMPDIR" => Some("/var/tmp".to_owned()),
        _ => None,
    };
    let paths = candidates(&env);
    assert_eq!(paths.len(), 10 * 6);
    assert_eq!(paths[0], PathBuf::from("/run/user/1000/discord-ipc-0"));
    assert!(paths.contains(&PathBuf::from(
        "/run/user/1000/app/com.discordapp.Discord/discord-ipc-3"
    )));
    assert!(paths.contains(&PathBuf::from("/run/user/1000/snap.discord/discord-ipc-0")));
    assert!(paths.contains(&PathBuf::from("/var/tmp/discord-ipc-9")));
    assert_eq!(paths.last(), Some(&PathBuf::from("/tmp/discord-ipc-9")));
}

#[cfg(unix)]
#[test]
fn without_a_runtime_dir_only_the_temp_dirs_remain_and_nothing_repeats() {
    let env = |name: &str| (name == "TMPDIR").then(|| "/tmp".to_owned());
    let paths = candidates(&env);
    assert_eq!(paths.len(), 10);
    assert_eq!(paths[0], PathBuf::from("/tmp/discord-ipc-0"));
}
