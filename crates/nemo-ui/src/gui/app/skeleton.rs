//! Which regions want a skeleton (skeleton-loading spec). The rules live here, one line per
//! region: loading, nothing cached, and on screen. [`crate::skeleton`] holds the timing.

use super::App;
use crate::skeleton::Region;

impl App {
    /// Tell each skeleton gate whether its region wants a skeleton now. Cheap; runs after every
    /// update and frame.
    pub(super) fn sync_skeletons(&mut self) {
        let reduced = self.ui.reduced_motion;
        let wants = [(Region::Detail, self.detail_loading())];
        for (region, wanted) in wants {
            self.skeletons.set(region, wanted, reduced);
        }
    }
}
