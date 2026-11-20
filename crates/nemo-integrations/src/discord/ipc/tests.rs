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

#[cfg(windows)]
#[test]
fn windows_paths_are_the_ten_named_pipes() {
    let paths = candidates(&|_| None);
    assert_eq!(paths.len(), 10);
    assert_eq!(paths[0], PathBuf::from(r"\\.\pipe\discord-ipc-0"));
    assert_eq!(paths[9], PathBuf::from(r"\\.\pipe\discord-ipc-9"));
}

#[cfg(unix)]
#[test]
fn without_a_runtime_dir_only_the_temp_dirs_remain_and_nothing_repeats() {
    let env = |name: &str| (name == "TMPDIR").then(|| "/tmp".to_owned());
    let paths = candidates(&env);
    assert_eq!(paths.len(), 10);
    assert_eq!(paths[0], PathBuf::from("/tmp/discord-ipc-0"));
}

/// A fake Discord on a Unix socket: answers `READY`, then every command with an empty answer.
#[cfg(unix)]
mod fake_server {
    use std::io::Write;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::thread::{self, JoinHandle};

    use super::super::{OP_FRAME, OP_PING, encode, read_frame};
    use crate::store::tests::TempDir;

    /// What the server read: `(opcode, body)` per frame.
    pub type Log = Arc<Mutex<Vec<(u32, String)>>>;

    pub struct Server {
        pub dir: TempDir,
        pub log: Log,
        pub thread: JoinHandle<()>,
    }

    impl Server {
        pub fn path(&self) -> PathBuf {
            self.dir.0.join("discord-ipc-0")
        }

        /// Start a server. It handles one connection, pings before each answer when `ping` is
        /// set, and stops after `answers` commands (closing the socket), or after it read the
        /// client's last frame when `linger` is set.
        pub fn start(ping: bool, answers: usize, linger: bool) -> Self {
            let dir = TempDir::new("ipc");
            std::fs::create_dir_all(&dir.0).expect("dir");
            let listener = UnixListener::bind(dir.0.join("discord-ipc-0")).expect("bind");
            let log: Log = Arc::default();
            let seen = Arc::clone(&log);
            let thread = thread::spawn(move || {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let Ok((op, body)) = read_frame(&mut stream) else {
                    return;
                };
                seen.lock()
                    .expect("lock")
                    .push((op, String::from_utf8_lossy(&body).into()));
                let ready = br#"{"cmd":"DISPATCH","evt":"READY","data":{}}"#;
                if stream.write_all(&encode(OP_FRAME, ready)).is_err() {
                    return;
                }
                for _ in 0..answers {
                    let Ok((op, body)) = read_frame(&mut stream) else {
                        return;
                    };
                    seen.lock()
                        .expect("lock")
                        .push((op, String::from_utf8_lossy(&body).into()));
                    if ping && op == OP_FRAME {
                        // The client must answer with a pong before its own answer arrives.
                        drop(stream.write_all(&encode(OP_PING, b"{\"n\":1}")));
                        let reply = read_frame(&mut stream);
                        if let Ok((op, body)) = reply {
                            seen.lock()
                                .expect("lock")
                                .push((op, String::from_utf8_lossy(&body).into()));
                        }
                    }
                    let answer = br#"{"cmd":"SET_ACTIVITY","evt":null,"data":{}}"#;
                    if stream.write_all(&encode(OP_FRAME, answer)).is_err() {
                        return;
                    }
                }
                if linger {
                    while let Ok((op, body)) = read_frame(&mut stream) {
                        seen.lock()
                            .expect("lock")
                            .push((op, String::from_utf8_lossy(&body).into()));
                    }
                }
            });
            Self { dir, log, thread }
        }
    }
}

#[cfg(unix)]
fn activity(details: &str) -> Activity {
    Activity {
        kind: ActivityKind::Playing,
        details: details.into(),
        state: None,
        start: None,
        end: None,
        large: None,
        small: None,
    }
}

#[cfg(unix)]
#[test]
fn a_client_handshakes_sets_an_activity_and_clears_it() {
    use super::{Connector, IpcConnector, OP_HANDSHAKE};
    let server = fake_server::Server::start(false, 2, true);
    let connector = IpcConnector::with_paths(vec![
        PathBuf::from("/nonexistent/discord-ipc-0"),
        server.path(),
    ]);
    let mut link = connector.open("42").expect("open");
    assert_eq!(
        link.set_activity(Some(&activity("Idle"))).expect("set"),
        Incoming::Other
    );
    assert_eq!(link.set_activity(None).expect("clear"), Incoming::Other);
    link.close();
    server.thread.join().expect("server");
    let log = server.log.lock().expect("lock").clone();
    assert_eq!(log[0].0, OP_HANDSHAKE);
    assert_eq!(log[0].1, r#"{"client_id":"42","v":1}"#);
    assert!(
        log[1].1.contains(r#""cmd":"SET_ACTIVITY""#) && log[1].1.contains(r#""details":"Idle""#)
    );
    assert!(!log[2].1.contains("activity"), "{}", log[2].1);
    // The close frame is the last one the client writes.
    assert_eq!(log.last().map(|f| f.0), Some(OP_CLOSE));
}

#[cfg(unix)]
#[test]
fn a_ping_gets_a_pong_before_the_answer() {
    use super::{Connector, IpcConnector, OP_PONG};
    let server = fake_server::Server::start(true, 1, false);
    let connector = IpcConnector::with_paths(vec![server.path()]);
    let mut link = connector.open("42").expect("open");
    assert_eq!(
        link.set_activity(Some(&activity("Idle"))).expect("set"),
        Incoming::Other
    );
    drop(link);
    server.thread.join().expect("server");
    let log = server.log.lock().expect("lock").clone();
    assert!(log.contains(&(OP_PONG, r#"{"n":1}"#.to_owned())), "{log:?}");
}

#[cfg(unix)]
#[test]
fn a_server_that_goes_away_is_an_error_and_the_next_open_works_again() {
    use super::{Connector, IpcConnector};
    let server = fake_server::Server::start(false, 1, false);
    let connector = IpcConnector::with_paths(vec![server.path()]);
    let mut link = connector.open("42").expect("open");
    assert!(link.set_activity(Some(&activity("One"))).is_ok());
    server.thread.join().expect("server");
    // The server closed its end after one answer: the next command fails.
    assert!(link.set_activity(Some(&activity("Two"))).is_err());
    drop(link);
    // Nobody listens any more: opening fails quietly.
    assert!(connector.open("42").is_err());
}

#[cfg(unix)]
#[test]
fn an_unreachable_discord_is_an_error_not_a_hang() {
    use super::{Connector, IpcConnector};
    let connector = IpcConnector::with_paths(vec![PathBuf::from("/nonexistent/discord-ipc-0")]);
    assert!(connector.open("42").is_err());
}
