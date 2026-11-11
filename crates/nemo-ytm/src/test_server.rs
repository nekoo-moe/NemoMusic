//! A tiny HTTP/1.1 server on 127.0.0.1 for tests. No network beyond loopback.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// What the server saw in one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Request path including the query.
    pub path: String,
    /// `Range: bytes=a-b` as `(a, Some(b))`, or `(a, None)` for an open end.
    pub range: Option<(u64, Option<u64>)>,
}

/// What the server answers.
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body,
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

/// Answer like a well-behaved static file server: 206 with `Content-Range`, 416 past the end.
pub fn range_response(data: &[u8], request: &Request) -> Response {
    let len = data.len() as u64;
    match request.range {
        None => Response::new(200, data.to_vec()),
        Some((start, _)) if start >= len => {
            Response::new(416, Vec::new()).header("Content-Range", &format!("bytes */{len}"))
        }
        Some((start, end)) => {
            let last = end.map_or(len - 1, |e| e.min(len - 1));
            let from = usize::try_from(start).expect("test offsets fit usize");
            let to = usize::try_from(last).expect("test offsets fit usize");
            Response::new(206, data[from..=to].to_vec())
                .header("Content-Range", &format!("bytes {start}-{last}/{len}"))
        }
    }
}

pub struct TestServer {
    port: u16,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn parse_range(value: &str) -> Option<(u64, Option<u64>)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    let (a, b) = spec.split_once('-')?;
    let start = a.trim().parse().ok()?;
    let end = if b.trim().is_empty() {
        None
    } else {
        Some(b.trim().parse().ok()?)
    };
    Some((start, end))
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let path = line.split_whitespace().nth(1)?.to_owned();
    let mut range = None;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("range")
        {
            range = parse_range(value);
        }
    }
    Some(Request { path, range })
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        206 => "Partial Content",
        302 => "Found",
        416 => "Range Not Satisfiable",
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
    /// Start serving. `handler` gets the request and the number of earlier requests.
    pub fn start<F>(handler: F) -> Self
    where
        F: Fn(&Request, usize) -> Response + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().expect("local addr").port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::clone(&requests);
        let stopped = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let Some(request) = read_request(&stream) else {
                    continue;
                };
                let index = {
                    let mut all = seen.lock().expect("lock");
                    all.push(request.clone());
                    all.len() - 1
                };
                let response = handler(&request, index);
                write_response(&stream, &response);
            }
        });
        Self {
            port,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().expect("lock").clone()
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
