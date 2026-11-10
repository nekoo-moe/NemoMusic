//! Local lyrics provider: a sidecar `.lrc` file, then lyrics embedded in the audio file's tags.
//!
//! Order (first hit wins):
//! 1. `<same stem>.lrc` next to the audio file (plain or enhanced LRC).
//! 2. Synced `ID3v2` `SYLT` frames in MP3 files (millisecond timestamps only; frame-based
//!    timestamps cannot be converted without the stream and are skipped). lofty 0.25.5 has a typed
//!    `SynchronizedTextFrame` but hands `SYLT` out as a binary frame, so it is parsed here.
//! 3. The generic lyrics item of the tags: `ID3v2` `USLT`, Vorbis `LYRICS`, MP4 `©lyr`. If the text
//!    holds LRC timestamps it becomes a synced doc, otherwise an unsynced one.
//!
//! Nothing here touches the network.

use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;

use lofty::config::ParseOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::id3::v2::{Frame, SyncTextContentType, SynchronizedTextFrame, TimestampFormat};
use lofty::mpeg::MpegFile;
use lofty::tag::{ItemKey, Tag};
use nemo_core::BoxFuture;
use nemo_core::lyrics::{
    Granularity, LyricLine, LyricsDoc, LyricsError, LyricsProvider, LyricsQuery, ProviderId,
};

use crate::lrc::parse_lrc;
use crate::plain::parse_plain;

/// Provider id shown in the UI.
pub const LOCAL_ID: ProviderId = ProviderId::new("Local file");

/// Lyrics next to or inside the user's own audio file.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalProvider;

impl LocalProvider {
    /// Create the provider.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl LyricsProvider for LocalProvider {
    fn id(&self) -> ProviderId {
        LOCAL_ID
    }

    fn hosts(&self) -> &'static [&'static str] {
        &[]
    }

    fn fetch<'a>(
        &'a self,
        query: &'a LyricsQuery,
    ) -> BoxFuture<'a, Result<Option<LyricsDoc>, LyricsError>> {
        Box::pin(async move {
            let Some(path) = query.local_path.clone() else {
                return Ok(None);
            };
            tokio::task::spawn_blocking(move || read_local(&path))
                .await
                .map_err(|_| LyricsError::Unavailable("local lyrics reader stopped".to_owned()))?
        })
    }
}

/// Read lyrics for a local audio file: sidecar first, then embedded tags. Blocking.
///
/// A file that cannot be parsed as audio gives `Ok(None)`, not an error.
///
/// # Errors
/// [`LyricsError::Unavailable`] if a sidecar exists but cannot be read.
pub fn read_local(audio_path: &Path) -> Result<Option<LyricsDoc>, LyricsError> {
    if let Some(doc) = read_sidecar(audio_path)? {
        return Ok(Some(doc));
    }
    Ok(read_embedded(audio_path))
}

fn read_sidecar(audio_path: &Path) -> Result<Option<LyricsDoc>, LyricsError> {
    let lrc_path = audio_path.with_extension("lrc");
    let bytes = match std::fs::read(&lrc_path) {
        Ok(b) => b,
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
            return Ok(None);
        }
        Err(e) => {
            return Err(LyricsError::Unavailable(format!(
                "could not read sidecar lyrics: {:?}",
                e.kind()
            )));
        }
    };
    Ok(text_to_doc(&String::from_utf8_lossy(&bytes)))
}

/// Text with LRC timestamps becomes a synced doc; other non-blank text becomes unsynced.
#[must_use]
pub fn text_to_doc(text: &str) -> Option<LyricsDoc> {
    let synced = parse_lrc(text, LOCAL_ID);
    if !synced.lines.is_empty() {
        return Some(synced);
    }
    if text.trim().is_empty() {
        return None;
    }
    let plain = parse_plain(text, LOCAL_ID);
    (!plain.lines.is_empty()).then_some(plain)
}

fn read_embedded(audio_path: &Path) -> Option<LyricsDoc> {
    if let Some(doc) = read_sylt(audio_path) {
        return Some(doc);
    }
    let tagged = match lofty::read_from_path(audio_path) {
        Ok(t) => t,
        Err(e) => {
            tracing::debug!(error = %e, "lyrics: could not read tags");
            return None;
        }
    };
    tagged.tags().iter().find_map(doc_from_tag)
}

/// Lyrics from the generic lyrics item of one tag (`USLT`, Vorbis `LYRICS`, MP4 lyrics).
#[must_use]
pub fn doc_from_tag(tag: &Tag) -> Option<LyricsDoc> {
    [ItemKey::Lyrics, ItemKey::UnsyncLyrics]
        .into_iter()
        .find_map(|key| tag.get_string(key).and_then(text_to_doc))
}

fn read_sylt(audio_path: &Path) -> Option<LyricsDoc> {
    let is_mp3 = audio_path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mp3"));
    if !is_mp3 {
        return None;
    }
    let mut file = File::open(audio_path).ok()?;
    let mpeg = MpegFile::read_from(&mut file, ParseOptions::new()).ok()?;
    let tag = mpeg.id3v2()?;
    tag.into_iter().find_map(|frame| match frame {
        Frame::Binary(b) if b.id().as_str() == "SYLT" => {
            let parsed = SynchronizedTextFrame::parse(&b.data, b.flags()).ok()?;
            doc_from_sylt(&parsed)
        }
        _ => None,
    })
}

/// Build a line-synced doc from a `SYLT` frame. Only lyrics frames with millisecond timestamps
/// count; blank lines are dropped.
#[must_use]
pub fn doc_from_sylt(frame: &SynchronizedTextFrame<'_>) -> Option<LyricsDoc> {
    if frame.timestamp_format != TimestampFormat::MS
        || frame.content_type != SyncTextContentType::Lyrics
    {
        return None;
    }
    let mut lines: Vec<LyricLine> = frame
        .content
        .iter()
        .filter(|(_, text)| !text.trim().is_empty())
        .map(|(ms, text)| LyricLine {
            start_ms: Some(u64::from(*ms)),
            end_ms: None,
            text: text.trim().to_owned(),
            words: Vec::new(),
        })
        .collect();
    if lines.is_empty() {
        return None;
    }
    lines.sort_by_key(|l| l.start_ms);
    Some(LyricsDoc {
        lines,
        granularity: Granularity::Line,
        provider: LOCAL_ID,
        matched: None,
        instrumental: false,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use lofty::TextEncoding;
    use lofty::config::WriteOptions;
    use lofty::id3::v2::{Id3v2Tag, UnsynchronizedTextFrame};
    use lofty::tag::{TagExt, TagType};

    use super::*;

    /// A fresh scratch directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "nemo-lyrics-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos())
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self, file: &str) -> PathBuf {
            self.0.join(file)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Three silent MPEG-1 Layer III frames (128 kbit/s, 44.1 kHz, 417 bytes each).
    fn fake_mp3_bytes() -> Vec<u8> {
        let mut out = Vec::new();
        for _ in 0..4 {
            let mut frame = vec![0u8; 417];
            frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
            out.extend_from_slice(&frame);
        }
        out
    }

    #[test]
    fn sidecar_lrc_is_read_first() {
        let dir = TempDir::new("sidecar");
        let audio = dir.path("song.flac");
        std::fs::write(&audio, b"not audio").expect("write");
        std::fs::write(
            dir.path("song.lrc"),
            "\u{feff}[ti:x]\r\n[00:01.00]first\r\n[00:03.50]second\r\n",
        )
        .expect("write");
        let doc = read_local(&audio).expect("ok").expect("some");
        assert_eq!(doc.provider, LOCAL_ID);
        assert_eq!(doc.granularity, Granularity::Line);
        assert_eq!(doc.lines.len(), 2);
        assert_eq!(doc.lines[1].start_ms, Some(3500));
    }

    #[test]
    fn sidecar_without_timestamps_is_unsynced() {
        let dir = TempDir::new("sidecar-plain");
        let audio = dir.path("song.ogg");
        std::fs::write(&audio, b"x").expect("write");
        std::fs::write(dir.path("song.lrc"), "just words\nmore words").expect("write");
        let doc = read_local(&audio).expect("ok").expect("some");
        assert_eq!(doc.granularity, Granularity::Unsynced);
        assert_eq!(doc.lines.len(), 2);
    }

    #[test]
    fn empty_sidecar_and_unreadable_audio_give_none() {
        let dir = TempDir::new("none");
        let audio = dir.path("song.mp3");
        std::fs::write(&audio, b"not audio at all").expect("write");
        assert_eq!(read_local(&audio), Ok(None));
        std::fs::write(dir.path("song.lrc"), "  \n").expect("write");
        assert_eq!(read_local(&audio), Ok(None));
        assert_eq!(read_local(&dir.path("missing.mp3")), Ok(None));
    }

    #[test]
    fn vorbis_lyrics_item_becomes_doc() {
        let mut tag = Tag::new(TagType::VorbisComments);
        tag.insert_text(ItemKey::Lyrics, "[00:02.00]sung line".to_owned());
        let doc = doc_from_tag(&tag).expect("some");
        assert_eq!(doc.granularity, Granularity::Line);
        assert_eq!(doc.lines[0].start_ms, Some(2000));

        let mut tag = Tag::new(TagType::VorbisComments);
        tag.insert_text(ItemKey::Lyrics, "plain words".to_owned());
        assert_eq!(
            doc_from_tag(&tag).map(|d| d.granularity),
            Some(Granularity::Unsynced)
        );
        assert!(doc_from_tag(&Tag::new(TagType::VorbisComments)).is_none());
    }

    #[test]
    fn sylt_frame_ms_becomes_line_doc() {
        let frame = SynchronizedTextFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            TimestampFormat::MS,
            SyncTextContentType::Lyrics,
            None,
            vec![
                (4000, "\nsecond".to_owned()),
                (1000, "first".to_owned()),
                (2000, "  ".to_owned()),
            ],
        );
        let doc = doc_from_sylt(&frame).expect("some");
        assert_eq!(doc.granularity, Granularity::Line);
        let texts: Vec<&str> = doc.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["first", "second"]);
        assert_eq!(doc.lines[0].start_ms, Some(1000));
    }

    #[test]
    fn sylt_frame_in_mpeg_frames_is_skipped() {
        let frame = SynchronizedTextFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            TimestampFormat::MPEG,
            SyncTextContentType::Lyrics,
            None,
            vec![(10, "x".to_owned())],
        );
        assert!(doc_from_sylt(&frame).is_none());
    }

    #[test]
    fn mp3_with_uslt_is_read() {
        let dir = TempDir::new("uslt");
        let audio = dir.path("song.mp3");
        std::fs::write(&audio, fake_mp3_bytes()).expect("write");
        let mut tag = Id3v2Tag::new();
        tag.insert(Frame::UnsynchronizedText(UnsynchronizedTextFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            String::new(),
            "unsynced one\nunsynced two".to_owned(),
        )));
        tag.save_to_path(&audio, WriteOptions::default())
            .expect("save tag");
        let doc = read_local(&audio).expect("ok").expect("some");
        assert_eq!(doc.granularity, Granularity::Unsynced);
        assert_eq!(doc.lines.len(), 2);
    }

    #[test]
    fn mp3_with_sylt_is_read_as_synced() {
        let dir = TempDir::new("sylt");
        let audio = dir.path("song.mp3");
        std::fs::write(&audio, fake_mp3_bytes()).expect("write");
        let sylt = SynchronizedTextFrame::new(
            TextEncoding::UTF8,
            *b"eng",
            TimestampFormat::MS,
            SyncTextContentType::Lyrics,
            None,
            vec![(1500, "one".to_owned()), (3000, "two".to_owned())],
        );
        let bytes = sylt.as_bytes(WriteOptions::default()).expect("encode");
        let mut tag = Id3v2Tag::new();
        tag.insert(Frame::Binary(lofty::id3::v2::BinaryFrame::new(
            lofty::id3::v2::FrameId::new("SYLT").expect("id"),
            bytes,
        )));
        tag.save_to_path(&audio, WriteOptions::default())
            .expect("save tag");
        let doc = read_local(&audio).expect("ok").expect("some");
        assert_eq!(doc.granularity, Granularity::Line);
        assert_eq!(doc.lines[1].start_ms, Some(3000));
    }

    #[test]
    fn provider_without_path_finds_nothing() {
        let q = LyricsQuery {
            title: "t".into(),
            artists: vec![],
            album: None,
            duration_ms: None,
            ytm_video_id: None,
            local_path: None,
        };
        let p = LocalProvider::new();
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        assert_eq!(rt.block_on(p.fetch(&q)), Ok(None));
        assert_eq!(p.id().as_str(), "Local file");
        assert_eq!(p.hosts().len(), 0);
    }
}
