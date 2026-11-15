//! Tests for the stored effects and transition settings.

use std::path::PathBuf;

use super::*;
use crate::config::{CONFIG_FILE, Settings};

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nemo-fx-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // A leftover from an earlier run is fine to drop.
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn all_off(settings: &Settings) -> bool {
    settings.effects.to_core() == EffectsSettings::default()
        && settings.transition.to_core() == TransitionSettings::default()
}

fn filter(gain_tenth_db: i16) -> StoredFilter {
    StoredFilter {
        kind: StoredFilterKind::HighShelf,
        freq_hz: 8000,
        gain_tenth_db,
        q_hundredths: 70,
    }
}

#[test]
fn the_defaults_are_the_core_defaults_with_everything_off() {
    let s = Settings::default();
    assert!(all_off(&s));
    let fx = s.effects.to_core();
    assert!(!fx.equalizer.enabled && !fx.delay.enabled && !fx.reverb.enabled);
    assert_eq!(s.transition.to_core().mode, TransitionMode::Off);
}

#[test]
fn effects_and_transition_round_trip_through_the_file() {
    let root = temp_root("roundtrip");
    let s = Settings {
        effects: StoredEffects {
            equalizer: StoredEq {
                enabled: true,
                preamp_tenth_db: -30,
                filters: vec![filter(45), filter(-120)],
            },
            delay: StoredDelay {
                enabled: true,
                time_ms: 400,
                feedback_pct: 50,
                mix_pct: 20,
            },
            reverb: StoredReverb {
                enabled: true,
                size_pct: 80,
                damping_pct: 10,
                mix_pct: 40,
            },
        },
        transition: StoredTransition {
            mode: StoredTransitionMode::Smart,
            seconds: 9,
            skip_fade: StoredSkipFade::Short,
            honour_gapless_marks: false,
        },
        ..Settings::default()
    };
    s.save(&root).expect("save");
    let loaded = Settings::load(&root);
    assert_eq!(loaded, s);
    let fx = loaded.effects.to_core();
    assert_eq!(fx.equalizer.filters[1], Some(EqFilter::from(filter(-120))));
    assert_eq!(fx.equalizer.filters[2], None);
    assert_eq!(loaded.transition.to_core().skip_fade, SkipFade::Short);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn an_old_file_or_a_partial_entry_leaves_every_effect_off() {
    let root = temp_root("old");
    std::fs::write(root.join(CONFIG_FILE), r#"{"volume_percent": 40}"#).expect("write");
    let old = Settings::load(&root);
    assert_eq!(old.volume_percent, 40);
    assert!(all_off(&old));
    // An entry that names values but not `enabled` stays off.
    std::fs::write(
        root.join(CONFIG_FILE),
        r#"{"effects": {"delay": {"time_ms": 500}, "equalizer": {"preamp_tenth_db": -20}},
            "transition": {"seconds": 4}}"#,
    )
    .expect("write");
    let partial = Settings::load(&root);
    let fx = partial.effects.to_core();
    assert!(!fx.delay.enabled && !fx.equalizer.enabled && !fx.reverb.enabled);
    assert_eq!(fx.delay.time_ms, 500);
    assert_eq!(partial.transition.to_core().mode, TransitionMode::Off);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn out_of_range_values_are_clamped() {
    let root = temp_root("clamp");
    let filters: Vec<String> = (0..14)
        .map(|i| {
            format!(
                r#"{{"freq_hz": 1000, "gain_tenth_db": {}}}"#,
                if i % 2 == 0 { 500 } else { -500 }
            )
        })
        .collect();
    let text = format!(
        r#"{{"effects": {{"equalizer": {{"filters": [{}]}},
                          "delay": {{"time_ms": 9000, "feedback_pct": 99, "mix_pct": 200}},
                          "reverb": {{"size_pct": 101, "damping_pct": 255, "mix_pct": 150}}}},
            "transition": {{"mode": "crossfade", "seconds": 30}}}}"#,
        filters.join(",")
    );
    std::fs::write(root.join(CONFIG_FILE), text).expect("write");
    let s = Settings::load(&root);
    let eq = &s.effects.equalizer;
    assert_eq!(eq.filters.len(), EQ_MAX_FILTERS);
    assert_eq!(eq.filters[0].gain_tenth_db, EqFilter::GAIN_MAX_TENTH_DB);
    assert_eq!(eq.filters[1].gain_tenth_db, EqFilter::GAIN_MIN_TENTH_DB);
    let d = s.effects.delay;
    assert_eq!(
        (d.time_ms, d.feedback_pct, d.mix_pct),
        (
            DelaySettings::TIME_MAX_MS,
            DelaySettings::FEEDBACK_MAX_PCT,
            100
        )
    );
    let r = s.effects.reverb;
    assert_eq!((r.size_pct, r.damping_pct, r.mix_pct), (100, 100, 100));
    assert_eq!(s.transition.seconds, TransitionSettings::SECONDS_MAX);

    std::fs::write(
        root.join(CONFIG_FILE),
        r#"{"effects": {"delay": {"time_ms": 0}}, "transition": {"seconds": 0}}"#,
    )
    .expect("write");
    let s = Settings::load(&root);
    assert_eq!(s.effects.delay.time_ms, DelaySettings::TIME_MIN_MS);
    assert_eq!(s.transition.seconds, TransitionSettings::SECONDS_MIN);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn core_settings_survive_the_stored_form() {
    let mut core = EffectsSettings::default();
    core.equalizer.enabled = true;
    core.equalizer.filters[0] = Some(EqFilter {
        kind: FilterKind::LowShelf,
        freq_hz: 105,
        gain_tenth_db: 60,
        q_hundredths: 71,
    });
    core.reverb.enabled = true;
    assert_eq!(StoredEffects::from(core).to_core(), core);
    let transition = TransitionSettings {
        mode: TransitionMode::Crossfade,
        seconds: 3,
        skip_fade: SkipFade::Short,
        honour_gapless_marks: false,
    };
    assert_eq!(StoredTransition::from(transition).to_core(), transition);
}
