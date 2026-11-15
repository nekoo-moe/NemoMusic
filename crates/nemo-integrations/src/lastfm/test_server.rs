//! A tiny HTTP/1.1 server on 127.0.0.1 that stands in for Last.fm in tests. Nothing beyond
//! loopback is contacted, and no real key, secret or session key is used.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// One request as the server saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// HTTP method, for example `POST`.
    pub verb: String,
    /// Path including the query.
    pub path: String,
    /// The `Content-Type` header.
    pub content_type: String,
    /// The form fields of the body, decoded, in order.
    pub form: Vec<(String, String)>,
}

impl Request {
    /// The value of the first form field named `name`.
    pub fn field(&self, name: &str) -> Option<&str> {
        self.form
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Decode `application/x-www-form-urlencoded` text.
fn decode_component(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).expect("ascii hex");
                out.push(u8::from_str_radix(hex, 16).expect("hex digit"));
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8(out).expect("form fields are UTF-8")
}

fn decode_form(body: &str) -> Vec<(String, String)> {
    body.split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let (n, v) = pair.split_once('=').unwrap_or((pair, ""));
            (decode_component(n), decode_component(v))
        })
        .collect()
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut words = line.split_whitespace();
    let verb = words.next()?.to_owned();
    let path = words.next()?.to_owned();
    let mut length = 0;
    let mut content_type = String::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().ok()?;
            } else if name.eq_ignore_ascii_case("content-type") {
                value.trim().clone_into(&mut content_type);
            }
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    let form = decode_form(std::str::from_utf8(&body).ok()?);
    Some(Request {
        verb,
        path,
        content_type,
        form,
    })
}

fn write_response(mut stream: &TcpStream, status: u16, body: &str) {
    let head = format!(
        "HTTP/1.1 {status} Status\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    // A client that hung up is not the test's concern.
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

/// Answers each request with `handler(request, index)`: an HTTP status and a body.
pub struct TestServer {
    port: u16,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl TestServer {
    pub fn start<F>(handler: F) -> Self
    where
        F: Fn(&Request, usize) -> (u16, String) + Send + 'static,
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
                let (status, body) = handler(&request, index);
                write_response(&stream, status, &body);
            }
        });
        Self {
            port,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/2.0/", self.port)
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
