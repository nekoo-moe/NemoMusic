//! First-connection cost of each host the first song needs (cold-start spec; development only,
//! real network). Run it as a fresh process: `cargo run -p nemo-ytm --example cold_net`.
//!
//! For each host it prints the DNS lookup, the TCP connect, the first HTTPS request on a new
//! client (client build, DNS, connect, TLS handshake and one round trip) and a second request on
//! the same client (one round trip on the open connection). The TLS handshake is about the first
//! request minus the second, minus DNS and connect. Two small requests per host, about eight in
//! all, no cookies. It never prints an address.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example reports what it measured"
)]

use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// The hosts of a first play: the native request, the catalog, the SponsorBlock lookup and the
/// yt-dlp download (first run only).
const HOSTS: &[(&str, &str)] = &[
    ("www.youtube.com", "/generate_204"),
    ("music.youtube.com", "/generate_204"),
    ("sponsor.ajay.app", "/"),
    ("github.com", "/"),
];

/// Pause between hosts, so the requests stay human-paced.
const PAUSE: Duration = Duration::from_secs(1);

fn ms(since: Instant) -> u128 {
    since.elapsed().as_millis()
}

fn probe(host: &str, path: &str) {
    let began = Instant::now();
    let addr = (host, 443_u16)
        .to_socket_addrs()
        .ok()
        .and_then(|mut found| found.next());
    let dns = ms(began);
    let Some(addr) = addr else {
        println!("{host}: DNS failed after {dns} ms");
        return;
    };
    let began = Instant::now();
    let tcp = TcpStream::connect_timeout(&addr, Duration::from_secs(8))
        .map(drop)
        .map_or_else(|_| "failed".to_owned(), |()| format!("{} ms", ms(began)));
    let began = Instant::now();
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        println!("{host}: no client");
        return;
    };
    let build = ms(began);
    let url = format!("https://{host}{path}");
    let first = Instant::now();
    let status = client.head(&url).send().map(|r| r.status().as_u16());
    let first_ms = ms(first);
    let second = Instant::now();
    let _ = client.head(&url).send();
    println!(
        "{host}: DNS {dns} ms | TCP {tcp} | client build {build} ms | first request {first_ms} ms \
         ({status:?}) | second request {} ms",
        ms(second)
    );
}

fn main() {
    println!("cold_net: a fresh process; the OS DNS cache may still hold the names");
    for (host, path) in HOSTS {
        probe(host, path);
        std::thread::sleep(PAUSE);
    }
}
