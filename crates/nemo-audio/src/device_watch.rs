//! Output device watcher: polls the device list in the background and sends updates to the
//! player control thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nemo_core::output::OutputDevice;

use crate::player::Msg;
use crate::sink::SinkFactory;

/// The default polling interval for device changes (1 second).
pub const WATCH_INTERVAL: Duration = Duration::from_secs(1);

/// Queries the system for available audio output devices.
pub trait DeviceEnumerator: Send + Sync {
    /// Return the list of current output devices.
    ///
    /// # Errors
    /// Returns an error message if the audio backend fails to enumerate devices.
    fn list_devices(&self) -> Result<Vec<OutputDevice>, String>;
}

impl<F> DeviceEnumerator for F
where
    F: Fn() -> Result<Vec<OutputDevice>, String> + Send + Sync,
{
    fn list_devices(&self) -> Result<Vec<OutputDevice>, String> {
        self()
    }
}

/// Adapts a [`SinkFactory`] into a [`DeviceEnumerator`].
#[derive(Clone)]
pub struct SinkFactoryEnumerator(pub Arc<dyn SinkFactory>);

impl DeviceEnumerator for SinkFactoryEnumerator {
    fn list_devices(&self) -> Result<Vec<OutputDevice>, String> {
        self.0.list_devices().map_err(|e| e.to_string())
    }
}

/// Queries `cpal` for available output devices.
#[derive(Debug, Default, Clone, Copy)]
pub struct CpalDeviceEnumerator;

impl DeviceEnumerator for CpalDeviceEnumerator {
    fn list_devices(&self) -> Result<Vec<OutputDevice>, String> {
        crate::cpal_sink::list_devices().map_err(|e| e.to_string())
    }
}

/// A background thread that polls output devices and sends updates to the player.
pub(crate) struct DeviceWatcher {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl DeviceWatcher {
    /// Spawn the `nemo-devices` watcher thread with a custom enumerator and interval.
    pub(crate) fn spawn_with(
        enumerator: Arc<dyn DeviceEnumerator>,
        tx: Sender<Msg>,
        interval: Duration,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("nemo-devices".into())
            .spawn(move || {
                run_watcher_loop(enumerator.as_ref(), &tx, &stop_thread, interval);
            })
            .ok();
        Self { stop, thread }
    }

    /// Stop the watcher thread and wait for it to finish.
    pub(crate) fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for DeviceWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The watcher thread body. Polls at `interval`, detecting changes and forwarding them.
fn run_watcher_loop(
    enumerator: &dyn DeviceEnumerator,
    tx: &Sender<Msg>,
    stop: &AtomicBool,
    interval: Duration,
) {
    let mut previous: Option<Vec<OutputDevice>> = None;
    while !stop.load(Ordering::SeqCst) {
        match enumerator.list_devices() {
            Ok(devices) => {
                if previous.as_ref() != Some(&devices) {
                    previous = Some(devices.clone());
                    if tx.send(Msg::DevicesChanged(devices)).is_err() {
                        // The player control thread has shut down.
                        return;
                    }
                }
            }
            Err(err) => {
                tracing::debug!(error = %err, "device enumeration failed");
            }
        }
        let start = Instant::now();
        while start.elapsed() < interval && !stop.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(10).min(interval));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nemo_core::output::DeviceId;
    use std::sync::Mutex;
    use std::sync::mpsc::channel;

    fn make_device(id: &str, name: &str, is_default: bool) -> OutputDevice {
        OutputDevice {
            id: DeviceId(id.into()),
            name: name.into(),
            is_default,
        }
    }

    #[test]
    fn watcher_detects_add_remove_and_default_change() {
        let (tx, rx) = channel();
        let devices = Arc::new(Mutex::new(vec![make_device("dev-1", "Device 1", true)]));
        let devices_clone = Arc::clone(&devices);
        let enumerator: Arc<dyn DeviceEnumerator> =
            Arc::new(move || Ok(devices_clone.lock().unwrap().clone()));

        let mut watcher = DeviceWatcher::spawn_with(enumerator, tx, Duration::from_millis(10));

        // Initial poll sends initial list
        let first = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("initial list");
        match first {
            Msg::DevicesChanged(d) => {
                assert_eq!(d.len(), 1);
                assert_eq!(d[0].id.0, "dev-1");
                assert!(d[0].is_default);
            }
            _ => panic!("unexpected message"),
        }

        // Add a device
        {
            let mut list = devices.lock().unwrap();
            list.push(make_device("dev-2", "Device 2", false));
        }
        let second = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("added device");
        match second {
            Msg::DevicesChanged(d) => {
                assert_eq!(d.len(), 2);
                assert_eq!(d[1].id.0, "dev-2");
            }
            _ => panic!("unexpected message"),
        }

        // Change default device
        {
            let mut list = devices.lock().unwrap();
            list[0].is_default = false;
            list[1].is_default = true;
        }
        let third = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("default change");
        match third {
            Msg::DevicesChanged(d) => {
                assert_eq!(d.len(), 2);
                assert!(!d[0].is_default);
                assert!(d[1].is_default);
            }
            _ => panic!("unexpected message"),
        }

        // Remove a device
        {
            let mut list = devices.lock().unwrap();
            list.remove(0);
        }
        let fourth = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("removed device");
        match fourth {
            Msg::DevicesChanged(d) => {
                assert_eq!(d.len(), 1);
                assert_eq!(d[0].id.0, "dev-2");
            }
            _ => panic!("unexpected message"),
        }

        watcher.stop();
    }

    #[test]
    fn watcher_handles_error_without_panic() {
        let (tx, rx) = channel();
        let fails = Arc::new(AtomicBool::new(true));
        let fails_clone = Arc::clone(&fails);
        let enumerator: Arc<dyn DeviceEnumerator> = Arc::new(move || {
            if fails_clone.load(Ordering::SeqCst) {
                Err("backend error".into())
            } else {
                Ok(vec![make_device("dev-1", "Device 1", true)])
            }
        });

        let mut watcher = DeviceWatcher::spawn_with(enumerator, tx, Duration::from_millis(10));

        // While failing, no message is sent
        assert!(rx.recv_timeout(Duration::from_millis(50)).is_err());

        // When recovered, sends the device list
        fails.store(false, Ordering::SeqCst);
        let msg = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("recovered list");
        match msg {
            Msg::DevicesChanged(d) => {
                assert_eq!(d.len(), 1);
                assert_eq!(d[0].id.0, "dev-1");
            }
            _ => panic!("unexpected message"),
        }

        watcher.stop();
    }
}
