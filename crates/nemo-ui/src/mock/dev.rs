//! A fake [`DevProbe`] with the data of an invented session: a few provider calls, one
//! rate-limited SoundCloud search, a stream that the native tier refused and `yt-dlp` gave, a
//! rejected sign-in check and some log lines. For the snapshots and the tests of the developer
//! panel. Nothing here is real: every host is a public one, no id or token appears.

use std::sync::Mutex;
use std::sync::PoisonError;

use nemo_core::dev::{
    DevAccount, DevConfigKey, DevEgress, DevLenClass, DevLevel, DevLogLine, DevPathKind, DevProbe,
    DevProvider, DevResolver, DevSnapshot, DevSolverState, DevSystem, DevTier, DevTraceRow,
};

/// 2026-11-20 08:30:00 UTC, in milliseconds since the Unix epoch.
pub const SAMPLE_NOW_MS: u64 = 1_795_163_400_000;

/// A fake probe over [`sample_snapshot`]. `clear` empties its trace and log.
#[derive(Debug, Default)]
pub struct MockProbe {
    cleared: Mutex<u32>,
}

impl MockProbe {
    /// How often `clear` was called.
    #[must_use]
    pub fn clears(&self) -> u32 {
        *self.cleared.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl DevProbe for MockProbe {
    fn snapshot(&self) -> DevSnapshot {
        let mut snap = sample_snapshot();
        if self.clears() > 0 {
            snap.trace.clear();
            snap.log.clear();
            snap.providers.clear();
        }
        snap
    }

    fn clear(&self) {
        *self.cleared.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    }

    fn redact(&self, text: &str) -> String {
        text.to_owned()
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "one fixed row of the sample trace"
)]
fn row(
    ago_s: u64,
    provider: &str,
    op: &str,
    host: &str,
    status: &str,
    ms: u64,
    tier: Option<&str>,
    rest_s: Option<u64>,
) -> DevTraceRow {
    let failed = status
        .parse::<u16>()
        .map_or(!matches!(status, "ok" | "hit" | "miss"), |c| c >= 400);
    DevTraceRow {
        at_ms: SAMPLE_NOW_MS - ago_s * 1000 - 123,
        provider: provider.to_owned(),
        op: op.to_owned(),
        host: host.to_owned(),
        status: status.to_owned(),
        ms: Some(ms),
        tier: tier.map(str::to_owned),
        rest_s,
        failed,
        ..DevTraceRow::default()
    }
}

fn log(ago_s: u64, level: DevLevel, target: &str, text: &str) -> DevLogLine {
    DevLogLine {
        at_ms: SAMPLE_NOW_MS - ago_s * 1000 - 456,
        level,
        target: target.to_owned(),
        text: text.to_owned(),
    }
}

fn trace() -> Vec<DevTraceRow> {
    let ytm = "music.youtube.com";
    let sc = "api-v2.soundcloud.com";
    vec![
        row(2, "soundcloud", "search", sc, "429", 212, None, Some(30)),
        row(9, "ytm", "search", ytm, "ok", 187, None, None),
        row(
            11,
            "resolver",
            "stream_url",
            "",
            "ok",
            3480,
            Some("yt_dlp"),
            None,
        ),
        row(14, "ytdlp", "run_fast", "", "ok", 3390, None, None),
        row(
            15,
            "resolver",
            "stream_url",
            "",
            "refused",
            104,
            Some("native"),
            None,
        ),
        row(
            15,
            "ytm",
            "player",
            "www.youtube.com",
            "200",
            98,
            None,
            None,
        ),
        row(15, "resolver", "cache", "", "miss", 0, None, None),
        row(
            31,
            "lyrics:lrclib",
            "lyrics",
            "lrclib.net",
            "404",
            241,
            None,
            None,
        ),
        row(
            33,
            "lyrics:simpmusic",
            "lyrics",
            "api-lyrics.simpmusic.org",
            "200",
            322,
            None,
            None,
        ),
        row(48, "ytm", "home_feed", ytm, "ok", 640, None, None),
        row(62, "soundcloud", "me", sc, "401", 156, None, None),
        row(
            63,
            "soundcloud",
            "signin_check",
            "",
            "rejected",
            0,
            None,
            None,
        ),
        row(
            70,
            "sponsorblock",
            "segments",
            "sponsor.ajay.app",
            "404",
            133,
            None,
            None,
        ),
    ]
}

fn providers() -> Vec<DevProvider> {
    let card =
        |name: &str, calls, errors, ok_ago: Option<u64>, err: Option<&str>, p50, p95| DevProvider {
            name: name.to_owned(),
            calls,
            errors,
            last_ok_ms: ok_ago.map(|s| SAMPLE_NOW_MS - s * 1000 - 123),
            last_error_kind: err.map(str::to_owned),
            p50_ms: Some(p50),
            p95_ms: Some(p95),
            resting_until_ms: None,
        };
    let mut sc = card("soundcloud", 9, 3, Some(64), Some("429"), 180, 410);
    sc.resting_until_ms = Some(SAMPLE_NOW_MS + 28_000);
    vec![
        card("lyrics:lrclib", 4, 1, Some(31), Some("404"), 230, 380),
        card("lyrics:simpmusic", 3, 0, Some(33), None, 300, 340),
        card("resolver", 6, 1, Some(11), Some("refused"), 104, 3480),
        sc,
        card("sponsorblock", 5, 2, Some(160), Some("404"), 120, 210),
        card("ytdlp", 2, 0, Some(14), None, 3390, 3400),
        card("ytm", 21, 0, Some(9), None, 190, 700),
    ]
}

fn resolver() -> DevResolver {
    let tier = |name: &str, ok, failed| DevTier {
        name: name.to_owned(),
        ok,
        failed,
    };
    DevResolver {
        tiers: vec![
            tier("native", 3, 2),
            tier("in_process", 0, 0),
            tier("yt_dlp", 2, 0),
        ],
        solver: DevSolverState::AutoOff,
        cache_hits: 4,
        cache_disk_hits: 1,
        cache_misses: 5,
        cache_size: None,
        ytdlp_path: DevPathKind::Managed,
        ytdlp_version: None,
        ytdlp_build: Some("3f9a0c71be42".to_owned()),
    }
}

fn accounts() -> Vec<DevAccount> {
    vec![
        DevAccount {
            site: "YouTube Music".to_owned(),
            signed_in: Some(true),
            token_present: Some(true),
            token_len: DevLenClass::Long,
            cookie_names: ["HSID", "SAPISID", "SID", "__Secure-3PAPISID"]
                .map(str::to_owned)
                .to_vec(),
            last_stage: Some("signin_store".to_owned()),
            last_outcome: Some("ok".to_owned()),
            last_at_ms: Some(SAMPLE_NOW_MS - 3_600_000),
        },
        DevAccount {
            site: "SoundCloud".to_owned(),
            signed_in: Some(false),
            token_present: Some(false),
            token_len: DevLenClass::Absent,
            cookie_names: Vec::new(),
            last_stage: Some("signin_check".to_owned()),
            last_outcome: Some("rejected".to_owned()),
            last_at_ms: Some(SAMPLE_NOW_MS - 63_000),
        },
    ]
}

fn system() -> DevSystem {
    let key = |k: &str, v: Option<&str>| DevConfigKey {
        key: k.to_owned(),
        value: v.map(str::to_owned),
    };
    let host = |h: &str, count| DevEgress {
        host: h.to_owned(),
        count,
    };
    DevSystem {
        version: "0.3.0".to_owned(),
        os: "linux x86_64".to_owned(),
        profile: "release".to_owned(),
        config: vec![
            key("volume_percent", Some("80")),
            key("theme", Some("moss")),
            key("in_process_resolver", Some("true")),
            key("music_folders", None),
            key("yt_dlp_path", None),
        ],
        egress: vec![
            host("music.youtube.com", 21),
            host("api-v2.soundcloud.com", 9),
            host("www.youtube.com", 6),
            host("lrclib.net", 4),
            host("sponsor.ajay.app", 5),
            host("api-lyrics.simpmusic.org", 3),
        ],
        uptime_s: 5_412,
        audio: Some("Speakers (USB DAC), 48000 Hz, 24 bit, 2 ch, Shared".to_owned()),
    }
}

/// The invented session.
#[must_use]
pub fn sample_snapshot() -> DevSnapshot {
    DevSnapshot {
        generation: 41,
        taken_ms: SAMPLE_NOW_MS,
        trace: trace(),
        log: vec![
            log(
                2,
                DevLevel::Warn,
                "nemo_soundcloud::client",
                "SoundCloud asked to slow down status=429",
            ),
            log(
                15,
                DevLevel::Info,
                "nemo_ytm::resolver",
                "native resolver rests after repeated refusals",
            ),
            log(
                48,
                DevLevel::Debug,
                "nemo_ytm::timing",
                "start stage stage=\"resolve native\" ok=false",
            ),
            log(
                63,
                DevLevel::Error,
                "nemo_soundcloud::account",
                "sign-in was not accepted",
            ),
            log(3_600, DevLevel::Info, "nemo_app", "starting version=0.3.0"),
        ],
        providers: providers(),
        resolver: resolver(),
        accounts: accounts(),
        system: system(),
    }
}
