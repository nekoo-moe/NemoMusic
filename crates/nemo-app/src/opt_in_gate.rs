//! The one check every opt-in service is built through (ADR 0004, "Opt-in settings pattern",
//! rule 4): its switch is on and the user confirmed the current version of its `OptInInfo`.
//!
//! A new host or new data raises the version, which closes the gate until the user confirms
//! again. `nemo-app` asks the gate before it builds a service, so a closed gate means no service
//! exists and nothing is requested.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the gate lands before the opt-in services that are built through it"
    )
)]

use std::collections::BTreeMap;

use nemo_core::opt_in::OptInInfo;

use crate::config::OptInChoice;

/// Every opt-in feature `nemo-app` builds through the gate. The task that wires a service adds
/// its `OptInInfo` here, and a test checks its hosts against the README egress list.
pub const OPT_INS: &[OptInInfo] = &[];

/// The stored choice for `info`, or its default when the user never touched the row. A
/// default-on row (OS media controls, which send nothing off the computer) counts as confirmed.
#[must_use]
pub fn choice(choices: &BTreeMap<String, OptInChoice>, info: &OptInInfo) -> OptInChoice {
    choices.get(info.id).copied().unwrap_or(OptInChoice {
        enabled: info.default_on,
        confirmed_version: if info.default_on { info.version } else { 0 },
    })
}

/// Whether the service of `info` may be built: the switch is on and the confirmed version is
/// the current one.
#[must_use]
pub fn is_open(choices: &BTreeMap<String, OptInChoice>, info: &OptInInfo) -> bool {
    let choice = choice(choices, info);
    choice.enabled && choice.confirmed_version == info.version
}

/// Build the service of `info` with `build` only when the gate is open; `build` is not called
/// otherwise.
pub fn build_if_open<T>(
    choices: &BTreeMap<String, OptInChoice>,
    info: &OptInInfo,
    build: impl FnOnce() -> T,
) -> Option<T> {
    is_open(choices, info).then(build)
}

#[cfg(test)]
mod tests;
