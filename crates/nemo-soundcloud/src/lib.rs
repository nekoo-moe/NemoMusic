//! SoundCloud as a second source next to YouTube Music (soundcloud-full spec, ADR 0005).
//!
//! The crate has its own client for the SoundCloud web API v2 (the calls soundcloud.com makes),
//! unofficial and off by default. It depends on `nemo-core` among the `nemo-*` crates and
//! carries its own small copy of the HTTP rules of `nemo-ytm`.
//!
//! - [`hosts`]: the hosts SoundCloud use reaches, shown before the first request.
//! - [`opt_in`]: the settings entry of the SoundCloud switch (off by default).
//! - [`http`]: the HTTP seam ([`ScHttp`]) and its `reqwest` implementation.
//! - [`client`]: the API client: client id, pacing, backoff, retries.
//! - [`catalog`]: [`SoundcloudCatalog`], search, track, playlist, album and user pages.

pub mod catalog;
pub mod client;
pub mod client_id;
pub mod hls;
pub mod home;
pub mod hosts;
pub mod http;
mod map;
mod model;
pub mod opener;
pub mod opt_in;
pub mod records;
pub mod stream;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod test_server;

pub use catalog::{Link, SoundcloudCatalog};
pub use client::{ClientError, Endpoints, Pacing, ScClient};
pub use hosts::HOSTS;
pub use http::{ReqwestHttp, ScHttp};
pub use opener::{ScStreamOpener, Shared};
pub use opt_in::OPT_IN;
pub use stream::{ScStreamResolver, streams};
