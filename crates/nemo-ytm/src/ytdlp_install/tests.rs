//! Unit tests for the release install code. No network.

use super::*;

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn the_checksum_line_of_the_asset_is_found() {
    let list = format!(
        "{}  yt-dlp\n{HASH}  yt-dlp_win.zip\n{}  yt-dlp_win_x86.zip\n",
        "f".repeat(64),
        "e".repeat(64)
    );
    assert_eq!(checksum_for(&list, ASSET).as_deref(), Some(HASH));
    let starred = format!("{}  *yt-dlp_win.zip\r\n", HASH.to_ascii_uppercase());
    assert_eq!(checksum_for(&starred, ASSET).as_deref(), Some(HASH));
    assert_eq!(checksum_for("abc  yt-dlp_win.zip", ASSET), None);
    assert_eq!(checksum_for(&format!("{HASH}  other.zip"), ASSET), None);
    assert_eq!(checksum_for("", ASSET), None);
}

#[test]
fn sha256_of_a_known_text() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn a_wrong_hash_is_refused() {
    let good = sha256_hex(b"abc");
    assert_eq!(verify(b"abc", &good), Ok(()));
    assert_eq!(verify(b"abd", &good), Err(InstallError::ChecksumMismatch));
    let list = format!("{good}  {LINUX_ASSET}\n");
    assert_eq!(checksum_for(&list, LINUX_ASSET), Some(good));
    assert_eq!(checksum_for(&list, ASSET), None);
}

#[test]
fn the_one_file_build_lands_through_a_temp_file() {
    let base = std::env::temp_dir().join(format!("nemo-onefile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("yt-dlp");
    let program = place_onefile(&dir, b"#!/bin/sh\n").expect("placed");
    assert_eq!(program, dir.join("yt-dlp"));
    assert_eq!(std::fs::read(&program).expect("read"), b"#!/bin/sh\n");
    assert!(!dir.join("yt-dlp.partial").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&program)
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
    // A second install replaces the first.
    place_onefile(&dir, b"v2").expect("replaced");
    assert_eq!(std::fs::read(&program).expect("read"), b"v2");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn the_program_folder_is_found_at_the_root_or_one_level_down() {
    let base = std::env::temp_dir().join(format!("nemo-onedir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let nested = base.join("yt-dlp_win");
    std::fs::create_dir_all(&nested).expect("dirs");
    assert_eq!(program_root(&base, "yt-dlp.exe"), None);
    std::fs::write(nested.join("yt-dlp.exe"), b"x").expect("file");
    assert_eq!(program_root(&base, "yt-dlp.exe"), Some(nested.clone()));
    std::fs::write(base.join("yt-dlp.exe"), b"x").expect("file");
    assert_eq!(program_root(&base, "yt-dlp.exe"), Some(base.clone()));
    let _ = std::fs::remove_dir_all(&base);
}

/// Two entries as the release zip has them: `yt-dlp_linux` (executable) and `_internal/`.
const FAKE_ZIP_HEX: &str = "504b03041400000000000000215c9f5b075e14000000140000000c00000079742d646c705f6c696e757823212f62696e2f73680a6563686f2066616b650a504b03041400000000000000215c8316dc8c0100000001000000120000005f696e7465726e616c2f626173652e74787478504b010214031400000000000000215c9f5b075e14000000140000000c0000000000000000000000ed010000000079742d646c705f6c696e7578504b010214031400000000000000215c8316dc8c0100000001000000120000000000000000000000a4013e0000005f696e7465726e616c2f626173652e747874504b050600000000020002007a0000006f0000000000";

fn fake_zip() -> Vec<u8> {
    (0..FAKE_ZIP_HEX.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&FAKE_ZIP_HEX[i..i + 2], 16).expect("hex"))
        .collect()
}

#[cfg(target_os = "linux")]
#[test]
fn the_linux_onedir_zip_is_unpacked_renamed_and_swapped_in() {
    use std::os::unix::fs::PermissionsExt;

    if !unzip_on_path() {
        return; // the install is not offered without `unzip`; nothing to test here
    }
    let base = std::env::temp_dir().join(format!("nemo-linux-onedir-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("yt-dlp");
    // An older install sits in the folder; it is replaced as a whole.
    std::fs::create_dir_all(&dir).expect("dirs");
    std::fs::write(dir.join("yt-dlp"), b"old one-file build").expect("old");
    std::fs::write(dir.join("old-only.txt"), b"x").expect("old");

    let program = unpack_linux_onedir(&dir, &fake_zip()).expect("unpacked");
    assert_eq!(program, dir.join("yt-dlp"));
    assert_eq!(
        std::fs::read(&program).expect("read"),
        b"#!/bin/sh\necho fake\n"
    );
    assert!(dir.join("_internal").join("base.txt").is_file());
    assert!(
        !dir.join("old-only.txt").exists(),
        "the old install is gone"
    );
    assert!(
        !dir.join("yt-dlp_linux").exists(),
        "renamed to the name the app looks for"
    );
    let mode = std::fs::metadata(&program)
        .expect("meta")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755);
    for leftover in ["yt-dlp.staging", "yt-dlp.old", "yt-dlp_linux.zip.partial"] {
        assert!(!base.join(leftover).exists(), "{leftover} is cleaned up");
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[cfg(target_os = "linux")]
#[test]
fn a_zip_that_is_not_one_leaves_the_old_install_alone() {
    if !unzip_on_path() {
        return;
    }
    let base = std::env::temp_dir().join(format!("nemo-linux-badzip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("yt-dlp");
    std::fs::create_dir_all(&dir).expect("dirs");
    std::fs::write(dir.join("yt-dlp"), b"old").expect("old");

    let err = unpack_linux_onedir(&dir, b"not a zip").expect_err("refused");
    assert!(matches!(err, InstallError::Unpack(_)), "{err}");
    assert_eq!(std::fs::read(dir.join("yt-dlp")).expect("old kept"), b"old");
    assert!(!base.join("yt-dlp.staging").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn the_linux_zip_has_its_own_checksum_line() {
    let hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let list = format!(
        "{hash}  yt-dlp_linux.zip\n{}  yt-dlp_linux\n",
        "f".repeat(64)
    );
    assert_eq!(checksum_for(&list, LINUX_ZIP_ASSET).as_deref(), Some(hash));
    assert_ne!(checksum_for(&list, LINUX_ASSET).as_deref(), Some(hash));
}
