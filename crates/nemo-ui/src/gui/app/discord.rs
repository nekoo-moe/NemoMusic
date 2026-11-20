//! The Discord presence switch as the app sees it (discord-presence spec): the opt-in choice and
//! `Hide song names on Discord` go to the integrations service, which keeps them; the window and
//! the sleep timer are reported to it so that it can show `Away`, `Browsing` and the timer.

use nemo_core::integrations::{BrowsingSection, IntegrationsService, WindowContext};
use nemo_core::opt_in::OptInInfo;

use super::App;
use super::message::Job;
use crate::route::Route;

/// Id of the Discord row in `IntegrationsService::infos`.
const DISCORD_ID: &str = "discord";

/// The notice of Discord presence, when the integrations offer it.
pub(super) fn find_info(service: &dyn IntegrationsService) -> Option<OptInInfo> {
    service
        .infos()
        .into_iter()
        .find(|info| info.id == DISCORD_ID)
}

/// Messages of the Discord row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscordMsg {
    /// The switch was turned on: open the panel. Still off.
    Ask,
    /// `Turn on` in the panel.
    TurnOn,
    /// `Cancel` in the panel.
    Cancel,
    /// The switch was turned off.
    TurnOff,
    /// `Hide song names on Discord` was flipped.
    Hide(bool),
}

impl App {
    /// Discord presence is on (switched on and confirmed).
    #[must_use]
    pub fn discord_on(&self) -> bool {
        self.backend.integrations.discord()
    }

    /// `Hide song names on Discord` is on.
    #[must_use]
    pub fn discord_hidden(&self) -> bool {
        self.backend.integrations.discord_private()
    }

    pub(super) fn discord_msg(&mut self, msg: DiscordMsg) -> Vec<Job> {
        let service = &self.backend.integrations;
        match msg {
            DiscordMsg::Ask => self.discord_confirming = true,
            DiscordMsg::Cancel => self.discord_confirming = false,
            DiscordMsg::TurnOn => {
                self.discord_confirming = false;
                service.set_discord(true);
            }
            DiscordMsg::TurnOff => {
                self.discord_confirming = false;
                service.set_discord(false);
            }
            DiscordMsg::Hide(on) => service.set_discord_private(on),
        }
        Vec::new()
    }

    /// Tell the integrations what the window shows. Cheap: the service ignores a call that
    /// repeats the last one.
    pub(super) fn sync_integrations(&self) {
        let section = match self.ui.route() {
            Route::Library => Some(BrowsingSection::Library),
            Route::Search(_) => Some(BrowsingSection::Search),
            _ => None,
        };
        self.backend.integrations.window_context(WindowContext {
            focused: self.focused,
            section,
            sleep_timer: self.sleep.is_some(),
        });
    }
}

#[cfg(test)]
mod tests;
