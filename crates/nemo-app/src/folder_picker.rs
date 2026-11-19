//! The `Add a folder` dialog (brand-intro spec, task 4): the operating system's folder dialog,
//! opened off the UI thread, and where its answer goes.
//!
//! A chosen folder is stored in `settings.json` (through the settings saver) and added to the
//! running library, which scans it in the background. A cancelled dialog changes nothing. The
//! folder's path reaches the screen as text and the saver as a path; no path or file name goes to
//! a log (only counts), see `ui_bridge`.

use std::future::Future;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use nemo_library::LocalCatalog;
use nemo_ui::gui::app::FolderPicker;

use crate::backend::rescan_in_background;
use crate::ui_bridge::SettingsMsg;

/// The running library, filled once the services are up.
pub type LibrarySlot = Arc<Mutex<Option<LocalCatalog>>>;

/// Where a chosen folder goes.
pub trait FolderSink: Send + Sync {
    /// Keep `path` and start watching it.
    fn add(&self, path: PathBuf);
}

/// The app's sink: the settings file, and the running library when it is up.
pub struct AppSink {
    saver: Sender<SettingsMsg>,
    library: LibrarySlot,
}

impl AppSink {
    /// A sink that stores through `saver` and adds to the library in `library`.
    #[must_use]
    pub fn new(saver: Sender<SettingsMsg>, library: LibrarySlot) -> Self {
        Self { saver, library }
    }
}

impl FolderSink for AppSink {
    fn add(&self, path: PathBuf) {
        // The saver only stops when the app exits; a failed send then is harmless.
        let _ = self.saver.send(SettingsMsg::AddFolder(path.clone()));
        let Ok(slot) = self.library.lock() else {
            return;
        };
        let Some(local) = slot.as_ref() else {
            return;
        };
        let added = local
            .library()
            .write()
            .is_ok_and(|mut library| library.add_folder(path));
        if added {
            rescan_in_background(local);
        }
    }
}

/// The picker the UI calls. `dialog` opens the dialog and resolves to the chosen folder (`None`
/// when cancelled); the folder goes to `sink` and comes back as text for the screen. A path that
/// is not absolute is ignored.
pub fn picker<D, F, S>(dialog: D, sink: Arc<S>) -> FolderPicker
where
    D: Fn() -> F + Send + Sync + 'static,
    F: Future<Output = Option<PathBuf>> + Send + 'static,
    S: FolderSink + 'static,
{
    FolderPicker::new(move || {
        let asked = dialog();
        let sink = Arc::clone(&sink);
        Box::pin(async move {
            let path = asked.await.filter(|p| p.is_absolute())?;
            sink.add(path.clone());
            Some(path.display().to_string())
        })
    })
}

/// The picker of the app: the native dialog, and the app's sink.
#[must_use]
pub fn native(saver: Sender<SettingsMsg>, library: LibrarySlot) -> FolderPicker {
    picker(native_dialog, Arc::new(AppSink::new(saver, library)))
}

/// The operating system's folder dialog (a native one on Windows, the desktop portal on Linux).
/// It runs on its own thread, so the UI keeps drawing while it is open.
async fn native_dialog() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Add a music folder")
        .pick_folder()
        .await
        .map(|handle| handle.path().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<PathBuf>>);

    impl FolderSink for Recorder {
        fn add(&self, path: PathBuf) {
            if let Ok(mut all) = self.0.lock() {
                all.push(path);
            }
        }
    }

    fn pick_with(answer: Option<PathBuf>) -> (Option<String>, Vec<PathBuf>) {
        let sink = Arc::new(Recorder::default());
        let ui = picker(
            move || std::future::ready(answer.clone()),
            Arc::clone(&sink),
        );
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread().build() else {
            return (None, Vec::new());
        };
        let shown = runtime.block_on(ui.pick());
        let stored = sink.0.lock().map(|all| all.clone()).unwrap_or_default();
        (shown, stored)
    }

    fn absolute(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn a_chosen_folder_goes_to_the_sink_and_comes_back_as_text() {
        let folder = absolute("music");
        let (shown, stored) = pick_with(Some(folder.clone()));
        assert_eq!(shown, Some(folder.display().to_string()));
        assert_eq!(stored, [folder]);
    }

    #[test]
    fn a_cancelled_dialog_changes_nothing() {
        let (shown, stored) = pick_with(None);
        assert_eq!(shown, None);
        assert!(stored.is_empty());
    }

    #[test]
    fn a_path_that_is_not_absolute_is_ignored() {
        let (shown, stored) = pick_with(Some(PathBuf::from("relative/music")));
        assert_eq!(shown, None);
        assert!(stored.is_empty());
    }
}
