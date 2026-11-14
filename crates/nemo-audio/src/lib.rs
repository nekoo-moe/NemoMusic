//! Audio engine: decode, ring buffer, device output, queue and signal-path report.
//!
//! Real-time code lives only in this crate (see `code.md`, "Real-time audio thread").
//!
//! Two ways to play:
//! - [`engine::Engine`] plays one fully decoded local file (the bit-exact reference path).
//! - [`player::Player`] is the streaming player: a queue of local files and YouTube streams,
//!   decoded incrementally on a feeder thread, with prefetch, seek, volume and SponsorBlock
//!   skipping.
//!
//! The default signal path has no resampling, channel mix or gain. Each of those is a
//! [`ProcessingStage`](nemo_core::audio::ProcessingStage) that exists only when the source
//! and the device disagree or the user lowered the volume, and it is reported in the signal
//! path.
#![cfg_attr(
    test,
    allow(
        clippy::float_cmp,
        reason = "the render tests check samples bit for bit, so exact float equality is the point"
    )
)]

pub mod block_on;
pub mod chain;
pub mod decoder;
pub mod engine;
pub mod mix;
pub mod pipeline;
pub mod player;
pub mod render;
pub mod resample;
pub mod ring;
pub mod sink;
pub mod source;
pub mod stream_decoder;

mod control;
mod control_cmd;
mod control_tick;
mod cpal_sink;
mod feeder;
mod prepare;
mod units;
