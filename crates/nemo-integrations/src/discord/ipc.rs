//! The local Discord IPC (discord-presence spec, criteria 13, 16, 19): the frame format, the
//! socket or pipe paths, and a connection with a reader thread.
//!
//! A frame is a `u32` opcode and a `u32` length (both little endian) and a JSON body. Safe code,
//! `std` only. The client asks and Discord answers, one frame at a time on one thread: a
//! Windows pipe handle cannot read and write at once, so there is no reader thread. A Unix
//! socket has a 5 s read and write timeout; a Windows pipe has none (Discord answers at once).

use std::io::{self, Read, Write};
use std::path::PathBuf;
#[cfg(unix)]
use std::time::Duration;

use serde_json::{Value, json};

use super::activity::Activity;

/// The first frame of a connection.
pub const OP_HANDSHAKE: u32 = 0;
/// A command or an event.
pub const OP_FRAME: u32 = 1;
/// Either side ends the connection.
pub const OP_CLOSE: u32 = 2;
/// Discord asks whether the client lives.
pub const OP_PING: u32 = 3;
/// The answer to a ping.
pub const OP_PONG: u32 = 4;
/// The longest body accepted, in bytes. A longer frame ends the connection.
pub const MAX_BODY: usize = 64 * 1024;
/// Longest wait for Discord's answer on a Unix socket.
#[cfg(unix)]
const ANSWER_WITHIN: Duration = Duration::from_secs(5);
/// Discord numbers its sockets `discord-ipc-0` to `discord-ipc-9`.
const SOCKETS: u32 = 10;

/// One frame, ready to write.
#[must_use]
pub fn encode(op: u32, body: &[u8]) -> Vec<u8> {
    let len = u32::try_from(body.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// Read one frame.
///
/// # Errors
/// The I/O error of the read, or `InvalidData` when the body is longer than [`MAX_BODY`].
pub fn read_frame(reader: &mut impl Read) -> io::Result<(u32, Vec<u8>)> {
    let mut head = [0_u8; 8];
    reader.read_exact(&mut head)?;
    let op = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
    let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
    let len = usize::try_from(len).unwrap_or(usize::MAX);
    if len > MAX_BODY {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too long"));
    }
    let mut body = vec![0_u8; len];
    reader.read_exact(&mut body)?;
    Ok((op, body))
}

/// The body of the handshake frame.
#[must_use]
pub fn handshake_body(client_id: &str) -> Vec<u8> {
    json!({"v": 1, "client_id": client_id})
        .to_string()
        .into_bytes()
}

/// The body of a `SET_ACTIVITY` command; `None` clears the activity.
#[must_use]
pub fn activity_body(pid: u32, nonce: u64, activity: Option<&Activity>) -> Vec<u8> {
    let mut args = json!({"pid": pid});
    if let (Some(activity), Some(map)) = (activity, args.as_object_mut()) {
        map.insert("activity".into(), activity.to_json());
    }
    json!({"cmd": "SET_ACTIVITY", "args": args, "nonce": nonce.to_string()})
        .to_string()
        .into_bytes()
}

/// What an incoming frame means to the worker. No text of the frame is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// Discord accepted the handshake.
    Ready,
    /// Discord refused something (a bad application id, a bad activity).
    Error,
    /// Discord asks for a pong with the same body.
    Ping(Vec<u8>),
    /// The connection ended, by Discord or by a read error.
    Close,
    /// Anything else (the answer to a good command, other events).
    Other,
}

/// Classify one frame.
#[must_use]
pub fn classify(op: u32, body: &[u8]) -> Incoming {
    match op {
        OP_PING => Incoming::Ping(body.to_vec()),
        OP_CLOSE => Incoming::Close,
        OP_FRAME => {
            let value: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
            let evt = value.get("evt").and_then(Value::as_str);
            let cmd = value.get("cmd").and_then(Value::as_str);
            match (cmd, evt) {
                (_, Some("ERROR")) => Incoming::Error,
                (Some("DISPATCH"), Some("READY")) => Incoming::Ready,
                _ => Incoming::Other,
            }
        }
        _ => Incoming::Other,
    }
}

/// The places Discord's socket or pipe may be, in the order to try.
///
/// `env` reads an environment variable. On Unix: the runtime dir, its Flatpak and Snap folders,
/// then the temp dirs; on Windows: the named pipes.
#[must_use]
pub fn candidates(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    if cfg!(windows) {
        return (0..SOCKETS)
            .map(|n| PathBuf::from(format!(r"\\.\pipe\discord-ipc-{n}")))
            .collect();
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(run) = env("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        let run = PathBuf::from(run);
        dirs.push(run.clone());
        dirs.push(run.join("app/com.discordapp.Discord"));
        dirs.push(run.join("app/com.discordapp.DiscordCanary"));
        dirs.push(run.join("snap.discord"));
    }
    for var in ["TMPDIR", "TMP", "TEMP"] {
        if let Some(dir) = env(var).filter(|d| !d.is_empty()) {
            dirs.push(PathBuf::from(dir));
        }
    }
    dirs.push(PathBuf::from("/tmp"));
    let mut out = Vec::new();
    for dir in dirs {
        for n in 0..SOCKETS {
            let path = dir.join(format!("discord-ipc-{n}"));
            if !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// The connection the worker uses: ask for an activity, close.
pub trait Link: Send {
    /// Set the activity, or clear it with `None`, and read Discord's answer.
    ///
    /// # Errors
    /// The I/O error when the connection is lost; the answer [`Incoming::Close`] counts as one.
    fn set_activity(&mut self, activity: Option<&Activity>) -> io::Result<Incoming>;

    /// Tell Discord the connection ends and drop it.
    fn close(self: Box<Self>);
}

/// Where connections come from. The real one is [`IpcConnector`]; tests use fakes.
pub trait Connector: Send + Sync {
    /// Open a connection and finish the handshake for `client_id` (Discord answered `READY`).
    ///
    /// # Errors
    /// The I/O error when no socket or pipe of Discord can be opened or Discord refuses.
    fn open(&self, client_id: &str) -> io::Result<Box<dyn Link>>;
}

/// An open socket or pipe.
struct Connection {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    shutdown: Box<dyn Fn() + Send>,
    nonce: u64,
}

impl Connection {
    fn send(&mut self, op: u32, body: &[u8]) -> io::Result<()> {
        self.writer.write_all(&encode(op, body))?;
        self.writer.flush()
    }

    /// Read until a frame that is not a ping; a ping gets its pong.
    fn answer(&mut self) -> io::Result<Incoming> {
        loop {
            let (op, body) = read_frame(&mut self.reader)?;
            match classify(op, &body) {
                Incoming::Ping(echo) => self.send(OP_PONG, &echo)?,
                Incoming::Close => {
                    return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "closed"));
                }
                other => return Ok(other),
            }
        }
    }

    fn handshake(&mut self, client_id: &str) -> io::Result<()> {
        self.send(OP_HANDSHAKE, &handshake_body(client_id))?;
        loop {
            match self.answer()? {
                Incoming::Ready => return Ok(()),
                Incoming::Error => {
                    return Err(io::Error::new(io::ErrorKind::ConnectionRefused, "refused"));
                }
                _ => {}
            }
        }
    }
}

impl Link for Connection {
    fn set_activity(&mut self, activity: Option<&Activity>) -> io::Result<Incoming> {
        self.nonce = self.nonce.wrapping_add(1);
        let body = activity_body(std::process::id(), self.nonce, activity);
        self.send(OP_FRAME, &body)?;
        self.answer()
    }

    fn close(mut self: Box<Self>) {
        // Best effort: the other side may be gone already.
        drop(self.send(OP_CLOSE, b"{}"));
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        (self.shutdown)();
    }
}

/// Opens the first of `paths` that Discord listens on.
#[derive(Debug, Clone)]
pub struct IpcConnector {
    paths: Vec<PathBuf>,
}

impl IpcConnector {
    /// The paths of this computer, from the environment.
    #[must_use]
    pub fn system() -> Self {
        Self::with_paths(candidates(&|name| std::env::var(name).ok()))
    }

    /// Try only `paths`, in order.
    #[must_use]
    pub const fn with_paths(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }
}

/// The three parts of an opened socket or pipe.
type Parts = (
    Box<dyn Read + Send>,
    Box<dyn Write + Send>,
    Box<dyn Fn() + Send>,
);

#[cfg(unix)]
fn open_path(path: &std::path::Path) -> io::Result<Parts> {
    use std::os::unix::net::UnixStream;
    let stream = UnixStream::connect(path)?;
    stream.set_read_timeout(Some(ANSWER_WITHIN))?;
    stream.set_write_timeout(Some(ANSWER_WITHIN))?;
    let reader = stream.try_clone()?;
    let writer = stream.try_clone()?;
    let closer = move || drop(stream.shutdown(std::net::Shutdown::Both));
    Ok((Box::new(reader), Box::new(writer), Box::new(closer)))
}

#[cfg(windows)]
fn open_path(path: &std::path::Path) -> io::Result<Parts> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    let reader = file.try_clone()?;
    // A pipe has no shutdown: dropping the handles closes it.
    Ok((Box::new(reader), Box::new(file), Box::new(|| {})))
}

#[cfg(not(any(unix, windows)))]
fn open_path(_path: &std::path::Path) -> io::Result<Parts> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "no local IPC on this system",
    ))
}

impl Connector for IpcConnector {
    fn open(&self, client_id: &str) -> io::Result<Box<dyn Link>> {
        let mut last = io::Error::new(io::ErrorKind::NotFound, "Discord is not running");
        for path in &self.paths {
            match open_path(path) {
                Ok((reader, writer, shutdown)) => {
                    let mut connection = Connection {
                        reader,
                        writer,
                        shutdown,
                        nonce: 0,
                    };
                    connection.handshake(client_id)?;
                    return Ok(Box::new(connection));
                }
                Err(err) => last = err,
            }
        }
        Err(last)
    }
}

#[cfg(test)]
mod tests;
