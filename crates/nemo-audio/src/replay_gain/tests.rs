use nemo_core::audio::ProcessingStage;
use nemo_core::library::{ReplayGainMode, ReplayGainValues};
use proptest::prelude::*;

use super::{ReplayGain, ReplayGainSettings, ReplayGainSwitch, db_to_scalar, scalar_to_db};

fn on(mode: ReplayGainMode, pre_gain_db: f32) -> ReplayGainSettings {
    ReplayGainSettings {
        enabled: true,
        mode,
        pre_gain_db,
    }
}

fn values(track: Option<f32>, album: Option<f32>) -> ReplayGainValues {
    ReplayGainValues {
        track_gain_db: track,
        album_gain_db: album,
        ..ReplayGainValues::default()
    }
}

fn db_of(settings: ReplayGainSettings, values: &ReplayGainValues) -> Option<f32> {
    ReplayGain::plan(&settings, values).map(|g| g.db())
}

fn close(got: Option<f32>, want: f32) -> bool {
    got.is_some_and(|db| (db - want).abs() < 1e-4)
}

#[test]
fn off_or_no_value_builds_no_stage() {
    let tagged = values(Some(-6.0), Some(-7.0));
    assert!(!ReplayGainSettings::default().enabled, "off by default");
    assert_eq!(
        ReplayGain::plan(&ReplayGainSettings::default(), &tagged),
        None
    );
    let track = on(ReplayGainMode::Track, 0.0);
    assert_eq!(ReplayGain::plan(&track, &ReplayGainValues::default()), None);
    assert_eq!(
        ReplayGain::plan(&track, &values(Some(f32::NAN), None)),
        None
    );
    assert_eq!(
        ReplayGain::plan(&on(ReplayGainMode::Track, -2.0), &values(Some(2.0), None)),
        None,
        "exactly 0 dB: no stage, no arithmetic"
    );
}

#[test]
fn the_mode_picks_the_gain_and_falls_back_to_the_other() {
    let both = values(Some(-6.0), Some(-8.0));
    assert!(close(db_of(on(ReplayGainMode::Track, 0.0), &both), -6.0));
    assert!(close(db_of(on(ReplayGainMode::Album, 0.0), &both), -8.0));
    let album_only = values(None, Some(-8.0));
    assert!(close(
        db_of(on(ReplayGainMode::Track, 0.0), &album_only),
        -8.0
    ));
    let track_only = values(Some(-6.0), None);
    assert!(close(
        db_of(on(ReplayGainMode::Album, 0.0), &track_only),
        -6.0
    ));
}

#[test]
fn the_pre_gain_is_added_and_limited() {
    let tagged = values(Some(-6.0), None);
    assert!(close(db_of(on(ReplayGainMode::Track, 3.5), &tagged), -2.5));
    assert!(close(
        db_of(on(ReplayGainMode::Track, -40.0), &tagged),
        -21.0
    ));
    assert!(close(
        db_of(on(ReplayGainMode::Track, f32::NAN), &tagged),
        -6.0
    ));
}

#[test]
fn the_peak_keeps_the_gain_at_or_below_full_scale() {
    let loud = ReplayGainValues {
        track_gain_db: Some(6.0),
        track_peak: Some(0.9),
        album_gain_db: Some(6.0),
        album_peak: Some(0.5),
    };
    let track = ReplayGain::plan(&on(ReplayGainMode::Track, 0.0), &loud).expect("a stage");
    assert!(
        (track.scalar() - 1.0 / 0.9).abs() < 1e-6,
        "lowered to 1 / peak"
    );
    let album = ReplayGain::plan(&on(ReplayGainMode::Album, 0.0), &loud).expect("a stage");
    assert!(
        close(Some(album.db()), 6.0),
        "0.5 x 2.0 stays at full scale"
    );
    assert_eq!(
        album.stage(),
        ProcessingStage::ReplayGain { db: album.db() }
    );
}

#[test]
fn apply_scales_every_sample() {
    let half = ReplayGain::plan(&on(ReplayGainMode::Track, 0.0), &values(Some(-6.0), None))
        .expect("a stage");
    let mut block = [0.5_f32, -1.0, 0.25];
    half.apply(&mut block);
    for (got, was) in block.iter().zip([0.5_f32, -1.0, 0.25]) {
        assert!((got - was * half.scalar()).abs() < 1e-7);
    }
}

#[test]
fn the_switch_is_shared_between_clones() {
    let app = ReplayGainSwitch::default();
    let player = app.clone();
    assert!(!player.get().enabled);
    app.set(on(ReplayGainMode::Album, 1.0));
    assert_eq!(player.get(), on(ReplayGainMode::Album, 1.0));
}

proptest! {
    #[test]
    fn db_and_scalar_convert_both_ways(db in -60.0_f32..60.0) {
        let back = scalar_to_db(db_to_scalar(db));
        prop_assert!((back - db).abs() < 1e-3, "{} -> {}", db, back);
        prop_assert!(db_to_scalar(db) > 0.0);
        prop_assert!(db_to_scalar(db + 0.1) > db_to_scalar(db), "rises with the gain");
    }

    #[test]
    fn peak_times_gain_never_exceeds_full_scale(
        gain in -30.0_f32..30.0,
        pre_gain in -20.0_f32..20.0,
        peak in 0.01_f32..2.0,
        album in any::<bool>(),
    ) {
        let mode = if album { ReplayGainMode::Album } else { ReplayGainMode::Track };
        let stored = ReplayGainValues {
            track_gain_db: Some(gain),
            track_peak: Some(peak),
            album_gain_db: Some(gain),
            album_peak: Some(peak),
        };
        if let Some(g) = ReplayGain::plan(&on(mode, pre_gain), &stored) {
            prop_assert!(g.scalar() * peak <= 1.0 + 1e-6, "{} x {}", g.scalar(), peak);
            prop_assert!((db_to_scalar(g.db()) - g.scalar()).abs() <= g.scalar() * 1e-4);
            let wanted = db_to_scalar(gain + pre_gain.clamp(-15.0, 15.0));
            prop_assert!(g.scalar() <= wanted * (1.0 + 1e-6), "never raised above the tag");
        }
    }
}
