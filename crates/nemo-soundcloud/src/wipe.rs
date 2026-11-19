//! The in-memory data of a SoundCloud session that sign-out must drop (criterion 12).
//!
//! Tracks the catalog mapped while someone was signed in can be private ones, so their stream
//! records, the kept stream addresses and the prefetched first segments go with the token.
//! Nothing here is on disk.

use std::sync::Arc;

use nemo_core::account::{SignOutWipe, WipeError, WipeScope};

use crate::client::ScClient;
use crate::stream::ScStreamResolver;

/// The sign-out step for the client's track records and the resolver's kept streams.
#[derive(Debug, Clone)]
pub struct SessionCaches {
    client: Arc<ScClient>,
    resolver: ScStreamResolver,
}

impl SessionCaches {
    /// The step for `client` and `resolver`.
    #[must_use]
    pub const fn new(client: Arc<ScClient>, resolver: ScStreamResolver) -> Self {
        Self { client, resolver }
    }
}

impl SignOutWipe for SessionCaches {
    fn name(&self) -> &'static str {
        "soundcloud-caches"
    }

    fn wipe(&self, _scope: WipeScope) -> Result<usize, WipeError> {
        Ok(self.client.records().clear() + self.resolver.forget_all())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nemo_core::account::{AccountId, WipeScope};
    use nemo_core::track::MediaUrl;

    use super::*;
    use crate::fake::{FakeHttp, ok};
    use crate::hosts::HOSTS;
    use crate::opener::Shared;
    use crate::records::TrackRecord;

    #[tokio::test]
    async fn the_step_forgets_records_and_prefetches_and_counts_them() {
        let http = Arc::new(FakeHttp::new(|_, _| ok("{}")));
        let client = Arc::new(ScClient::new(http));
        let resolver = ScStreamResolver::new(
            Arc::clone(&client),
            Arc::new(Shared::new()),
            tokio::runtime::Handle::current(),
        );
        let page = MediaUrl::parse("https://soundcloud.com/a/b", HOSTS).expect("page");
        client.records().insert(
            page.as_str(),
            TrackRecord {
                id: 1,
                transcodings: Vec::new(),
                authorization: None,
                preview: false,
            },
        );
        let step = SessionCaches::new(Arc::clone(&client), resolver);
        assert_eq!(step.name(), "soundcloud-caches");
        assert_eq!(step.wipe(WipeScope::Account(AccountId(0))), Ok(1));
        assert!(client.records().is_empty());
        assert_eq!(step.wipe(WipeScope::All), Ok(0));
    }
}
