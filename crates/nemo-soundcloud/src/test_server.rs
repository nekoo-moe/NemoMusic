//! A tiny HTTP/1.1 server on 127.0.0.1 for tests. No network beyond loopback. A small copy in
//! the style of `nemo-ytm/src/test_server.rs` (crates must not depend on each other).

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// What the server answers.
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

pub struct TestServer {
    port: u16,
    paths: Arc<Mutex<Vec<String>>>,
    /// The `Authorization` header of each request, in order.
    auths: Arc<Mutex<Vec<Option<String>>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// The request path (with its query) and the value of its `Authorization` header, if any.
fn read_request(stream: &TcpStream) -> Option<(String, Option<String>)> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let path = line.split_whitespace().nth(1)?.to_owned();
    let mut auth = None;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        if header.trim_end().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("authorization")
        {
            auth = Some(value.trim().to_owned());
        }
    }
    Some((path, auth))
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        302 => "Found",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Status",
    }
}

fn write_response(mut stream: &TcpStream, response: &Response) {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        reason(response.status),
        response.body.len()
    );
    for (name, value) in &response.headers {
        let _ = write!(head, "{name}: {value}\r\n");
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&response.body);
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

impl TestServer {
    /// Start serving. `handler` gets the request path and the number of earlier requests.
    pub fn start<F>(handler: F) -> Self
    where
        F: Fn(&str, usize) -> Response + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().expect("local addr").port();
        let paths = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let auths = Arc::new(Mutex::new(Vec::new()));
        let seen_auths = Arc::clone(&auths);
        let seen = Arc::clone(&paths);
        let stopped = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let Some((path, auth)) = read_request(&stream) else {
                    continue;
                };
                seen_auths.lock().expect("lock").push(auth);
                let index = {
                    let mut all = seen.lock().expect("lock");
                    all.push(path.clone());
                    all.len() - 1
                };
                write_response(&stream, &handler(&path, index));
            }
        });
        Self {
            port,
            paths,
            auths,
            stop,
            thread: Some(thread),
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub fn paths(&self) -> Vec<String> {
        self.paths.lock().expect("lock").clone()
    }

    /// The `Authorization` header of every request so far, in order.
    pub fn authorizations(&self) -> Vec<Option<String>> {
        self.auths.lock().expect("lock").clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so the thread can end.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
