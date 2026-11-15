use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use nemo_core::effects::{EqFilter, EqSettings, FilterKind};

use super::{BUILTIN_PRESETS, BuiltinPreset, PRESET_FILE_MAX_BYTES, PresetError, PresetStore};
use crate::effects::{EqProfile, ProfileError, TEN_BAND_HZ, write_autoeq};

/// A fresh, empty folder under the system temp folder, removed when dropped.
struct TempFolder(PathBuf);

impl TempFolder {
    fn new(tag: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("nemo-presets-{tag}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir); // a leftover from an earlier crashed run
        fs::create_dir_all(&dir).expect("create temp folder");
        Self(dir)
    }

    fn store(&self) -> PresetStore {
        PresetStore::new(self.0.join("presets"))
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0); // best effort; the OS cleans temp later
    }
}

fn profile(name: &str, gain_tenth_db: i16) -> EqProfile {
    EqProfile {
        name: name.to_owned(),
        preamp_tenth_db: -35,
        filters: vec![
            EqFilter {
                kind: FilterKind::LowShelf,
                freq_hz: 105,
                gain_tenth_db,
                q_hundredths: 70,
            },
            EqFilter {
                kind: FilterKind::Peak,
                freq_hz: 3_000,
                gain_tenth_db: -20,
                q_hundredths: 141,
            },
        ],
    }
}

fn io_kind(result: Result<impl std::fmt::Debug, PresetError>) -> Option<io::ErrorKind> {
    match result {
        Err(PresetError::Io(e)) => Some(e.kind()),
        _ => None,
    }
}

#[test]
fn flat_sets_every_band_and_the_preamp_to_zero() {
    let before = EqSettings {
        enabled: true,
        preamp_tenth_db: -60,
        filters: BuiltinPreset::BASS_BOOST.filters(),
    };
    let flat = BuiltinPreset::FLAT.apply(before);
    assert!(flat.enabled, "picking a preset keeps the on switch");
    assert_eq!(flat.preamp_tenth_db, 0);
    let bands: Vec<_> = flat.filters.iter().flatten().collect();
    assert_eq!(bands.len(), 10);
    assert!(bands.iter().all(|f| f.gain_tenth_db == 0));
    assert!(flat.filters[10..].iter().all(Option::is_none));
    let off = BuiltinPreset::VOCAL.apply(EqSettings::default());
    assert!(
        !off.enabled,
        "picking a preset never turns the equalizer on"
    );
}

#[test]
fn builtin_presets_are_ten_band_and_in_range() {
    let names: Vec<_> = BUILTIN_PRESETS.iter().map(|p| p.name).collect();
    assert_eq!(names, ["Flat", "Bass boost", "Treble boost", "Vocal"]);
    for preset in BUILTIN_PRESETS {
        let p = preset.profile();
        assert_eq!(p.name, preset.name);
        assert_eq!(p.preamp_tenth_db, 0);
        let freqs: Vec<_> = p.filters.iter().map(|f| f.freq_hz).collect();
        assert_eq!(freqs, TEN_BAND_HZ);
        assert!(p.filters.iter().all(|f| {
            (EqFilter::GAIN_MIN_TENTH_DB..=EqFilter::GAIN_MAX_TENTH_DB).contains(&f.gain_tenth_db)
        }));
        let text = write_autoeq(&p);
        assert_eq!(crate::effects::parse_autoeq(preset.name, &text), Ok(p));
    }
}

#[test]
fn save_list_load_rename_and_delete() {
    let tmp = TempFolder::new("actions");
    let store = tmp.store();
    assert_eq!(
        store.list().ok(),
        Some(Vec::new()),
        "missing folder is empty"
    );

    let warm = profile("warm", 40);
    store.save(&warm).expect("save warm");
    store.save(&profile("Bright", -10)).expect("save Bright");
    fs::write(store.folder().join("notes.md"), "not a preset").expect("write");
    assert_eq!(
        store.list().ok(),
        Some(vec!["Bright".to_owned(), "warm".to_owned()])
    );
    assert_eq!(store.load("warm").ok(), Some(warm.clone()));

    store.rename("warm", "Warm room").expect("rename");
    let renamed = store.load("Warm room").expect("load renamed");
    assert_eq!(renamed.name, "Warm room");
    assert_eq!(renamed.filters, warm.filters);
    assert_eq!(io_kind(store.load("warm")), Some(io::ErrorKind::NotFound));

    store.delete("Bright").expect("delete");
    assert_eq!(store.list().ok(), Some(vec!["Warm room".to_owned()]));
    assert_eq!(
        io_kind(store.delete("Bright")),
        Some(io::ErrorKind::NotFound)
    );
}

#[test]
fn a_taken_name_is_refused_and_the_file_is_kept() {
    let tmp = TempFolder::new("taken");
    let store = tmp.store();
    store.save(&profile("Mine", 40)).expect("save");
    store.save(&profile("Other", 10)).expect("save");
    assert!(matches!(
        store.save(&profile("Mine", -40)),
        Err(PresetError::NameTaken)
    ));
    assert!(matches!(
        store.rename("Other", "Mine"),
        Err(PresetError::NameTaken)
    ));
    assert_eq!(store.load("Mine").ok(), Some(profile("Mine", 40)));
    assert_eq!(store.load("Other").ok(), Some(profile("Other", 10)));
}

#[test]
fn import_names_the_preset_after_the_file_and_export_writes_it_back() {
    let tmp = TempFolder::new("import");
    let store = tmp.store();
    let source = tmp.0.join("My Headphones ParametricEQ.txt");
    fs::write(
        &source,
        "Preamp: -3.5 dB\nFilter 1: ON LSC Fc 105 Hz Gain 4.0 dB Q 0.70\n\
         Filter 2: ON PK Fc 3000 Hz Gain -2.0 dB Q 1.41\n",
    )
    .expect("write source");
    let imported = store.import(&source).expect("import");
    assert_eq!(imported, profile("My Headphones ParametricEQ", 40));
    assert_eq!(
        store.list().ok(),
        Some(vec!["My Headphones ParametricEQ".to_owned()])
    );
    assert!(matches!(store.import(&source), Err(PresetError::NameTaken)));

    let out = tmp.0.join("exported.txt");
    store
        .export("My Headphones ParametricEQ", &out)
        .expect("export");
    let text = fs::read_to_string(&out).expect("read export");
    assert_eq!(text, write_autoeq(&imported));
}

#[test]
fn a_refused_profile_is_not_saved() {
    let tmp = TempFolder::new("refused");
    let store = tmp.store();
    let source = tmp.0.join("Lowpass.txt");
    fs::write(&source, "Filter 1: ON LP Fc 100 Hz Gain 0.0 dB Q 0.70\n").expect("write");
    assert!(matches!(
        store.import(&source),
        Err(PresetError::Profile(ProfileError::UnsupportedFilter {
            line: 1,
            ..
        }))
    ));
    assert_eq!(store.list().ok(), Some(Vec::new()));
}

#[test]
fn files_over_64_kib_are_refused() {
    let tmp = TempFolder::new("cap");
    let store = tmp.store();
    let line = "# padding comment line\n";
    let mut text = String::from("Preamp: -1.0 dB\n");
    while text.len() as u64 <= PRESET_FILE_MAX_BYTES {
        text.push_str(line);
    }
    let big = tmp.0.join("Big.txt");
    fs::write(&big, &text).expect("write big");
    assert_eq!(
        io_kind(store.import(&big)),
        Some(io::ErrorKind::FileTooLarge)
    );

    // Exactly at the cap is still read.
    text.truncate(usize::try_from(PRESET_FILE_MAX_BYTES).expect("64 KiB fits usize"));
    let cut = text.rfind('\n').expect("a newline") + 1;
    text.truncate(cut);
    while (text.len() as u64) < PRESET_FILE_MAX_BYTES {
        text.push(' ');
    }
    let fits = tmp.0.join("Fits.txt");
    fs::write(&fits, &text).expect("write fits");
    assert_eq!(text.len() as u64, PRESET_FILE_MAX_BYTES);
    assert!(store.import(&fits).is_ok());

    let binary = tmp.0.join("Binary.txt");
    fs::write(&binary, [0xff, 0xfe, 0x00, 0x50]).expect("write binary");
    assert_eq!(
        io_kind(store.import(&binary)),
        Some(io::ErrorKind::InvalidData)
    );
}

#[test]
fn names_that_cannot_be_file_names_are_refused() {
    let tmp = TempFolder::new("names");
    let store = tmp.store();
    for bad in [
        "",
        " lead",
        "trail ",
        ".hidden",
        "dot.",
        "a/b",
        "a\\b",
        "..",
        "C:x",
        "what?",
        "con",
        "LPT1.backup",
        "tab\there",
    ] {
        assert_eq!(
            io_kind(store.save(&profile(bad, 10))),
            Some(io::ErrorKind::InvalidInput),
            "{bad:?}"
        );
        assert_eq!(io_kind(store.load(bad)), Some(io::ErrorKind::InvalidInput));
    }
    let long = "x".repeat(101);
    assert_eq!(
        io_kind(store.save(&profile(&long, 10))),
        Some(io::ErrorKind::InvalidInput)
    );
    assert!(store.save(&profile("Đàn bầu v1.2", 10)).is_ok());
    assert_eq!(store.list().ok(), Some(vec!["Đàn bầu v1.2".to_owned()]));
}
