//! Byte sources for the decoder: an adapter from [`ByteSource`] to symphonia's `MediaSource`, and
//! a local-file source.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use nemo_core::services::ByteSource;
use symphonia::core::io::MediaSource;

/// Lets symphonia read any [`ByteSource`] (an HTTP range reader, a file, a memory buffer).
///
/// Reads and seeks go straight to the inner source, so the decode thread blocks where the
/// network blocks and the audio callback never does.
pub struct MediaAdapter {
    inner: Box<dyn ByteSource>,
}

impl MediaAdapter {
    /// Wrap a byte source.
    #[must_use]
    pub fn new(inner: Box<dyn ByteSource>) -> Self {
        Self { inner }
    }
}

impl Read for MediaAdapter {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Seek for MediaAdapter {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl MediaSource for MediaAdapter {
    /// What the inner source reports, so a live stream is never seeked.
    fn is_seekable(&self) -> bool {
        self.inner.is_seekable()
    }

    fn byte_len(&self) -> Option<u64> {
        self.inner.byte_len()
    }
}

/// Opening a [`FileSource`] failed. The text never holds the path, so it is safe to log and show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FileOpenError {
    /// No file exists at the path.
    #[error("the audio file does not exist")]
    NotFound,
    /// The file exists but may not be read.
    #[error("no permission to read the audio file")]
    PermissionDenied,
    /// Any other failure, by its I/O error kind.
    #[error("the audio file could not be opened ({0:?})")]
    Other(io::ErrorKind),
}

impl From<io::Error> for FileOpenError {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::NotFound => Self::NotFound,
            io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            kind => Self::Other(kind),
        }
    }
}

/// A file on this computer as a [`ByteSource`].
///
/// The one reader for every file the player opens: local library files and songs saved by the
/// offline cache. Pass it to [`StreamDecoder::open`](crate::stream_decoder::StreamDecoder::open)
/// with no container hint; the probe finds the format from the bytes. Reads and seeks block, so
/// it is used on the prepare and decode threads only, never in the audio callback.
pub struct FileSource {
    file: File,
    len: Option<u64>,
}

impl FileSource {
    /// Open a file for reading. The length is read once, here.
    ///
    /// # Errors
    /// [`FileOpenError`] when the file cannot be opened; it holds no path.
    pub fn open(path: &Path) -> Result<Self, FileOpenError> {
        let file = File::open(path)?;
        let len = file.metadata().ok().map(|m| m.len());
        Ok(Self { file, len })
    }
}

impl Read for FileSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl Seek for FileSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.file.seek(pos)
    }
}

impl ByteSource for FileSource {
    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}

/// A byte buffer in memory as a [`ByteSource`]. Used for fixtures and small resources.
pub struct MemorySource {
    cursor: io::Cursor<Vec<u8>>,
}

impl MemorySource {
    /// Serve `bytes` as a seekable source.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            cursor: io::Cursor::new(bytes),
        }
    }
}

impl Read for MemorySource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.cursor.read(buf)
    }
}

impl Seek for MemorySource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(pos)
    }
}

impl ByteSource for MemorySource {
    fn byte_len(&self) -> Option<u64> {
        Some(self.cursor.get_ref().len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_reads_seeks_and_reports_length() {
        let mut a = MediaAdapter::new(Box::new(MemorySource::new(vec![1, 2, 3, 4, 5])));
        assert!(a.is_seekable());
        assert_eq!(a.byte_len(), Some(5));
        let mut two = [0_u8; 2];
        a.read_exact(&mut two).expect("read");
        assert_eq!(two, [1, 2]);
        a.seek(SeekFrom::Start(4)).expect("seek");
        a.read_exact(&mut two[..1]).expect("read");
        assert_eq!(two[0], 5);
    }

    /// A live stream: reads work, but it must not be seeked.
    struct LiveSource(MemorySource);

    impl Read for LiveSource {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
    }

    impl Seek for LiveSource {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.0.seek(pos)
        }
    }

    impl ByteSource for LiveSource {
        fn byte_len(&self) -> Option<u64> {
            None
        }

        fn is_seekable(&self) -> bool {
            false
        }
    }

    #[test]
    fn adapter_forwards_a_non_seekable_source() {
        let a = MediaAdapter::new(Box::new(LiveSource(MemorySource::new(vec![1, 2]))));
        assert!(!a.is_seekable());
        assert_eq!(a.byte_len(), None);
    }

    #[test]
    fn sources_are_seekable_by_default() {
        let f = MemorySource::new(Vec::new());
        assert!(ByteSource::is_seekable(&f));
    }
}
