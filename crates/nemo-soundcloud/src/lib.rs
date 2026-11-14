//! SoundCloud as a second catalog next to YouTube Music: public tracks only, no sign-in
//! (soundcloud spec, ADR 0004).
//!
//! This crate never starts a process or opens a socket. Search, pages and streams go through
//! the `nemo-core` URL-resolver trait, which `nemo-ytm` implements with `yt-dlp`. It depends on
//! `nemo-core` only.
//!
//! - [`hosts`]: the hosts SoundCloud use reaches, shown before the first request.
//! - [`opt_in`]: the settings entry of the SoundCloud switch (off by default).

pub mod hosts;
pub mod opt_in;

pub use hosts::HOSTS;
pub use opt_in::OPT_IN;
