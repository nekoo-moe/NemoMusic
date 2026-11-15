//! The delay and reverb stages of the effects chain: each with its on/off ramp, and a
//! parameter change that fades the stage out, switches and fades it back in.

use super::delay::{Delay, DelayParams};
use super::fade::Fade;
use super::reverb::{Reverb, ReverbParams};

/// A delay or reverb as one stage of the chain. Every method forwards to the stage's own
/// method of the same name.
///
/// RT-SAFE: each forwarded method is RT-SAFE (see [`Delay`] and [`Reverb`]).
pub(super) trait Effect {
    type Params: Copy + PartialEq;
    fn params(&self) -> Self::Params;
    fn set_params(&mut self, params: Self::Params);
    fn process(&mut self, block: &mut [f32]);
    fn reset(&mut self);
    fn tail_frames(&self) -> u64;
}

impl Effect for Delay {
    type Params = DelayParams;
    fn params(&self) -> DelayParams {
        Self::params(self)
    }
    fn set_params(&mut self, params: DelayParams) {
        Self::set_params(self, params);
    }
    fn process(&mut self, block: &mut [f32]) {
        Self::process(self, block);
    }
    fn reset(&mut self) {
        Self::reset(self);
    }
    fn tail_frames(&self) -> u64 {
        Self::params(self).tail_frames()
    }
}

impl Effect for Reverb {
    type Params = ReverbParams;
    fn params(&self) -> ReverbParams {
        Self::params(self)
    }
    fn set_params(&mut self, params: ReverbParams) {
        Self::set_params(self, params);
    }
    fn process(&mut self, block: &mut [f32]) {
        Self::process(self, block);
    }
    fn reset(&mut self) {
        Self::reset(self);
    }
    fn tail_frames(&self) -> u64 {
        Self::params(self).tail_frames()
    }
}

/// A delay or reverb with its on/off ramp and a parameter change waiting for the ramp.
#[derive(Debug)]
pub(super) struct Stage<E: Effect> {
    pub(super) effect: E,
    enabled: bool,
    fade: Fade,
    pending: Option<E::Params>,
}

impl<E: Effect> Stage<E> {
    pub(super) fn new(effect: E, rate: u32) -> Self {
        Self {
            effect,
            enabled: false,
            fade: Fade::new(rate),
            pending: None,
        }
    }

    /// Take the new on switch and parameters. A silent stage switches at once; a sounding
    /// stage with new parameters fades out first.
    ///
    /// RT-SAFE: copies and comparisons. No allocation, lock or logging.
    pub(super) fn apply(&mut self, on: bool, params: E::Params) {
        if self.fade.is_off() {
            if on {
                self.effect.reset();
            }
            self.effect.set_params(params);
        } else if on && params != self.effect.params() {
            self.pending = Some(params);
            self.enabled = true;
            self.fade.set(false);
            return;
        }
        self.enabled = on;
        self.fade.set(on);
    }

    /// Run the stage on `chunk`, ramped against the dry copy in `dry` while it fades.
    ///
    /// RT-SAFE: works in the preallocated `dry` scratch. No allocation, lock or logging.
    pub(super) fn process(&mut self, chunk: &mut [f32], dry: &mut [f32], channels: usize) {
        if self.fade.is_off() {
            return;
        }
        if self.fade.is_full() {
            self.effect.process(chunk);
        } else if let Some(dry) = dry.get_mut(..chunk.len()) {
            dry.copy_from_slice(chunk);
            self.effect.process(chunk);
            blend(chunk, dry, channels, &mut self.fade);
        }
        if self.fade.is_off()
            && let Some(params) = self.pending.take()
        {
            self.effect.set_params(params);
            self.fade.set(self.enabled);
        }
    }

    /// The stage is silent and has nothing waiting.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    pub(super) const fn is_idle(&self) -> bool {
        self.fade.is_off() && self.pending.is_none()
    }

    /// Every ramp of the stage has finished.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    pub(super) const fn is_settled(&self) -> bool {
        self.fade.is_settled() && self.pending.is_none()
    }

    /// Frames the stage rings after its input stops; 0 while it is silent.
    ///
    /// RT-SAFE: reads only. No allocation, lock or logging.
    pub(super) fn tail_frames(&self) -> u64 {
        if self.fade.is_off() {
            0
        } else {
            self.effect.tail_frames()
        }
    }
}

/// `wet = dry + gain * (wet - dry)` frame by frame, stepping `fade` once per frame.
///
/// RT-SAFE: in-place arithmetic. No allocation, lock or logging.
fn blend(wet: &mut [f32], dry: &[f32], channels: usize, fade: &mut Fade) {
    for (w, d) in wet
        .chunks_exact_mut(channels)
        .zip(dry.chunks_exact(channels))
    {
        let gain = fade.next_gain();
        if gain >= 1.0 {
            continue; // the end of the ramp: the stage output as it is
        }
        for (s, x) in w.iter_mut().zip(d) {
            *s = if gain <= 0.0 { *x } else { x + gain * (*s - x) };
        }
    }
}
