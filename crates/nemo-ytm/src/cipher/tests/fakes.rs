//! Fakes for the player source, the solver and the `player` request. No network, no engine.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use nemo_core::BoxFuture;
use nemo_core::stream::StreamError;
use nemo_core::track::VideoId;

use crate::cipher::{
    ChallengeSolver, Challenges, PlayerRequest, PlayerSource, Script, SolveError, Solved,
};

/// A player source with a settable id that counts what it was asked.
pub struct FakeSource {
    pub id: Mutex<String>,
    pub ids: AtomicUsize,
    pub scripts: AtomicUsize,
    pub fail_ids: AtomicBool,
    pub delay: Duration,
    /// The id of the last script that was asked for.
    pub last_script: Mutex<Option<String>>,
}

impl FakeSource {
    pub fn new(id: &str, delay: Duration) -> Arc<Self> {
        Arc::new(Self {
            id: Mutex::new(id.to_owned()),
            ids: AtomicUsize::new(0),
            scripts: AtomicUsize::new(0),
            fail_ids: AtomicBool::new(false),
            delay,
            last_script: Mutex::new(None),
        })
    }

    pub fn id_asked_for_script(&self) -> Option<String> {
        self.last_script
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set_id(&self, id: &str) {
        *self.id.lock().unwrap_or_else(PoisonError::into_inner) = id.to_owned();
    }
}

impl PlayerSource for FakeSource {
    fn latest_id(&self) -> BoxFuture<'_, Result<String, StreamError>> {
        Box::pin(async move {
            self.ids.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            if self.fail_ids.load(Ordering::SeqCst) {
                return Err(StreamError::Network("no".to_owned()));
            }
            Ok(self
                .id
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone())
        })
    }

    fn script<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<String, StreamError>> {
        Box::pin(async move {
            self.scripts.fetch_add(1, Ordering::SeqCst);
            *self
                .last_script
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(id.to_owned());
            tokio::time::sleep(self.delay).await;
            Ok(format!("var a={{signatureTimestamp:20522}}; /* {id} */"))
        })
    }
}

/// A solver that answers `N<challenge>` and `S<challenge>` and counts its jobs.
#[derive(Default)]
pub struct FakeSolver {
    pub wholes: AtomicUsize,
    pub solves: AtomicUsize,
    pub fail: AtomicBool,
}

impl ChallengeSolver for FakeSolver {
    fn solve(
        &self,
        script: Script<'_>,
        challenges: &Challenges,
        _limit: Duration,
    ) -> Result<Solved, SolveError> {
        match script {
            Script::Whole { id, .. } => {
                self.wholes.fetch_add(1, Ordering::SeqCst);
                Ok(Solved {
                    preprocessed: Some(format!("pre-{id}")),
                    ..Solved::default()
                })
            }
            Script::Preprocessed { .. } => {
                self.solves.fetch_add(1, Ordering::SeqCst);
                if self.fail.load(Ordering::SeqCst) {
                    return Err(SolveError::NoFunction("n"));
                }
                Ok(Solved {
                    n: challenges
                        .n
                        .iter()
                        .map(|c| (c.clone(), format!("N{c}")))
                        .collect(),
                    sig: challenges
                        .sig
                        .iter()
                        .map(|c| (c.clone(), format!("S{c}")))
                        .collect(),
                    preprocessed: None,
                })
            }
        }
    }
}

/// A `player` request that answers with a fixed text and remembers the timestamp it was given.
pub struct FakeRequest {
    pub answer: String,
    pub asked: AtomicUsize,
    pub sts: Mutex<Option<u32>>,
}

impl FakeRequest {
    pub fn new(answer: &str) -> Arc<Self> {
        Arc::new(Self {
            answer: answer.to_owned(),
            asked: AtomicUsize::new(0),
            sts: Mutex::new(None),
        })
    }
}

impl PlayerRequest for FakeRequest {
    fn ask<'a>(
        &'a self,
        _video: &'a VideoId,
        sts: u32,
    ) -> BoxFuture<'a, Result<String, StreamError>> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        *self.sts.lock().unwrap_or_else(PoisonError::into_inner) = Some(sts);
        let answer = self.answer.clone();
        Box::pin(async move { Ok(answer) })
    }
}

/// A `player` answer: a ciphered Opus format, a plain AAC format and a protected one.
pub const ANSWER: &str = r#"{"playabilityStatus":{"status":"OK"},"streamingData":{"adaptiveFormats":[
 {"mimeType":"audio/webm; codecs=\"opus\"","bitrate":130000,"audioSampleRate":"48000","audioChannels":2,
  "signatureCipher":"s=ENC&sp=sig&url=https%3A%2F%2Fr1.example.invalid%2Fvideoplayback%3Fexpire%3D1900000000%26n%3Dabc%26itag%3D251"},
 {"mimeType":"audio/mp4; codecs=\"mp4a.40.2\"","bitrate":128000,"audioSampleRate":"44100","audioChannels":2,
  "url":"https://r1.example.invalid/videoplayback?expire=1900000000&n=zzz&itag=140"},
 {"mimeType":"audio/webm; codecs=\"opus\"","bitrate":999000,"drmFamilies":["WIDEVINE"],
  "signatureCipher":"s=XXX&sp=sig&url=https%3A%2F%2Fr1.example.invalid%2Fvideoplayback%3Fitag%3D774"}
]}}"#;
