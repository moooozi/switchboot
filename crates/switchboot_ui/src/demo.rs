//! In-memory demo backend. No firmware, network or installer access.
use crate::{Backend, Entry, Shortcut, Snapshot, UpdateProgress, UpdateRelease};
use std::sync::{Arc, Mutex};

/// Safe in-memory backend for UI development, including a sample update preview.
/// It never accesses the network, launches an installer, touches firmware or restarts.
pub struct DemoBackend {
    state: Mutex<Snapshot>,
}

impl Default for DemoBackend {
    fn default() -> Self {
        Self {
            state: Mutex::new(Snapshot {
                entries: vec![
                    Entry {
                        id: 0,
                        description: "Windows Boot Manager".into(),
                    },
                    Entry {
                        id: 1,
                        description: "Fedora".into(),
                    },
                    Entry {
                        id: 2,
                        description: "Ubuntu".into(),
                    },
                ],
                order: vec![0, 1],
                boot_next: None,
                current: Some(0),
                firmware_setup: false,
                portable: false,
            }),
        }
    }
}

impl Backend for DemoBackend {
    fn updates_supported(&self) -> bool {
        true
    }

    fn check_update(&self) -> Result<Option<UpdateRelease>, String> {
        Ok(Some(UpdateRelease {
            version: "0.4.0".into(),
            notes: "Demo update preview\nImproved boot-entry management and Windows updates."
                .into(),
        }))
    }

    fn install_update(&self, _: String) -> Result<(), String> {
        Err("Demo preview only. No update was downloaded or installed.".into())
    }

    fn install_update_with_progress(
        &self,
        version: String,
        progress: Arc<dyn Fn(UpdateProgress) + Send + Sync>,
    ) -> Result<(), String> {
        for step in 0..=40 {
            progress(UpdateProgress {
                downloaded: step * 80_000,
                total: Some(3_200_000),
            });
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        self.install_update(version)
    }

    fn load(&self) -> Result<Snapshot, String> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|error| error.to_string())
    }
    fn discover(&self) -> Result<Vec<Entry>, String> {
        Ok(self.load()?.entries)
    }
    fn set_order(&self, order: Vec<u16>) -> Result<(), String> {
        self.state.lock().map_err(|error| error.to_string())?.order = order;
        Ok(())
    }
    fn set_next(&self, next: Option<u16>) -> Result<(), String> {
        self.state
            .lock()
            .map_err(|error| error.to_string())?
            .boot_next = next;
        Ok(())
    }
    fn set_firmware(&self, enabled: bool) -> Result<(), String> {
        self.state
            .lock()
            .map_err(|error| error.to_string())?
            .firmware_setup = enabled;
        Ok(())
    }
    fn create_shortcut(&self, _: Shortcut) -> Result<(), String> {
        Ok(())
    }
    fn restart(&self) -> Result<(), String> {
        Ok(())
    }
}
