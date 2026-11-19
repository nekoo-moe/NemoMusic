//! The tier between the plain native request and `yt-dlp`: ask the `tv` client, solve what its
//! answer carries in process, give the address.
//!
//! The order of a resolve is: the player script is ready (kept, or prepared once, see
//! [`PlayerStore`]); one `player` request with the script's signature timestamp; the signature
//! and the `n` value are solved by the embedded engine; the address is put together. Nothing
//! here logs an address, an id or a script.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nemo_core::BoxFuture;
use nemo_core::audio::Codec;
use nemo_core::stream::{ResolvedStream, StreamError, StreamResolver};
use nemo_core::track::VideoId;
use tokio::runtime::Handle;

use super::player::{HttpPlayerSource, PlayerInfo, PlayerStore};
use super::solver::{ChallengeSolver, Challenges, JsSolver, Script, SolveError, Solved};
use crate::http::InitError;
use crate::innertube::{TV, Visitor, parse_player_ciphered, send};
use crate::timing;

/// Time limit of a solve against an already preprocessed player.
const SOLVE_LIMIT: Duration = Duration::from_secs(10);

/// Asks for the `player` answer of one video.
pub trait PlayerRequest: Send + Sync {
    /// The answer text for `video`, asked with the player's signature timestamp `sts`.
    fn ask<'a>(
        &'a self,
        video: &'a VideoId,
        sts: u32,
    ) -> BoxFuture<'a, Result<String, StreamError>>;
}

/// The real request: the `tv` client over HTTPS, no cookie, no PO token.
#[derive(Debug, Clone)]
pub struct TvRequest {
    client: reqwest::Client,
    visitor: Visitor,
}

impl TvRequest {
    /// Build the HTTP client.
    ///
    /// # Errors
    /// [`InitError::Client`] when it cannot be built.
    pub fn new() -> Result<Self, InitError> {
        let client = reqwest::Client::builder()
            .connect_timeout(crate::innertube::PLAYER_TIMEOUT)
            .timeout(crate::innertube::PLAYER_TIMEOUT)
            .redirect(crate::http::https_redirects())
            .https_only(true)
            .build()
            .map_err(|_| InitError::Client)?;
        Ok(Self {
            client,
            visitor: Visitor::default(),
        })
    }
}

impl PlayerRequest for TvRequest {
    fn ask<'a>(
        &'a self,
        video: &'a VideoId,
        sts: u32,
    ) -> BoxFuture<'a, Result<String, StreamError>> {
        Box::pin(async move {
            let (mut text, retry) =
                send(&self.client, &TV, video, &self.visitor, Some(sts)).await?;
            if retry {
                // The first answer refused and gave visitor data: ask once more with it.
                text = send(&self.client, &TV, video, &self.visitor, Some(sts))
                    .await?
                    .0;
            }
            Ok(text)
        })
    }
}

/// Resolves streams through the `tv` client and the embedded solver.
pub struct CipherResolver {
    store: Arc<PlayerStore>,
    solver: Arc<dyn ChallengeSolver>,
    request: Arc<dyn PlayerRequest>,
    allowed: Option<Vec<Codec>>,
}

impl std::fmt::Debug for CipherResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CipherResolver").finish_non_exhaustive()
    }
}

impl CipherResolver {
    /// The real resolver: its player cache file is `player_file` (`None`: memory only), and it
    /// picks formats whose codec is in `allowed`.
    ///
    /// # Errors
    /// [`InitError::Client`] when an HTTP client cannot be built; the solver thread failing to
    /// start counts as the same.
    pub fn new(
        handle: &Handle,
        player_file: Option<PathBuf>,
        allowed: &[Codec],
    ) -> Result<Arc<Self>, InitError> {
        let solver: Arc<dyn ChallengeSolver> =
            Arc::new(JsSolver::start().map_err(|_| InitError::Client)?);
        let store = PlayerStore::new(
            Arc::new(HttpPlayerSource::new()?),
            Arc::clone(&solver),
            handle.clone(),
            player_file,
        );
        Ok(Arc::new(Self::with_parts(
            store,
            solver,
            Arc::new(TvRequest::new()?),
            allowed,
        )))
    }

    /// A resolver from its parts (tests give fakes).
    #[must_use]
    pub fn with_parts(
        store: Arc<PlayerStore>,
        solver: Arc<dyn ChallengeSolver>,
        request: Arc<dyn PlayerRequest>,
        allowed: &[Codec],
    ) -> Self {
        Self {
            store,
            solver,
            request,
            allowed: Some(allowed.to_vec()),
        }
    }

    /// The player cache, for the start-up preparation and the sign-out wipe.
    #[must_use]
    pub fn store(&self) -> &Arc<PlayerStore> {
        &self.store
    }

    /// Prepare the player in the background when an earlier run left one in its file, so the
    /// first resolve that needs it does not wait. Makes no request when the file is recent.
    pub fn warm_from_file(&self, handle: &Handle) {
        if self.store.has_file() {
            let store = Arc::clone(&self.store);
            drop(handle.spawn(async move {
                let _ = store.get().await;
            }));
        }
    }

    async fn solve(
        &self,
        player: PlayerInfo,
        challenges: Challenges,
    ) -> Result<Solved, StreamError> {
        if challenges == Challenges::default() {
            return Ok(Solved::default());
        }
        let solver = Arc::clone(&self.solver);
        tokio::task::spawn_blocking(move || {
            let script = Script::Preprocessed {
                id: &player.id,
                text: &player.preprocessed,
            };
            solver.solve(script, &challenges, SOLVE_LIMIT)
        })
        .await
        .map_err(|_| StreamError::Network("solver task ended".to_owned()))?
        .map_err(|e: SolveError| StreamError::Parse(e.to_string()))
    }

    async fn run(&self, video: &VideoId) -> Result<ResolvedStream, StreamError> {
        let began = Instant::now();
        let player = self.store.get().await?;
        timing::stage("cipher player", began, true);
        let asking = Instant::now();
        let text = self.request.ask(video, player.sts).await?;
        timing::stage("cipher player request", asking, true);
        let pending = parse_player_ciphered(&text, self.allowed.as_deref())?;
        let solving = Instant::now();
        let solved = self.solve(player, pending.challenges()).await;
        timing::stage("cipher solve", solving, solved.is_ok());
        match solved {
            Ok(solved) => pending.finish(&solved),
            Err(e) => {
                // The table may be stale: read the current player id again next time.
                self.store.refresh();
                Err(e)
            }
        }
    }
}

impl StreamResolver for CipherResolver {
    fn resolve<'a>(
        &'a self,
        video: &'a VideoId,
    ) -> BoxFuture<'a, Result<ResolvedStream, StreamError>> {
        Box::pin(self.run(video))
    }

    /// A stream built here could not be opened (a CDN 403): the player table may be stale.
    fn stream_failed(&self, _video: &VideoId) {
        self.store.refresh();
    }
}
