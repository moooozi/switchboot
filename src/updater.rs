//! Adapts application policy and updater types to the UI backend boundary.
use std::path::PathBuf;
use std::sync::Mutex;
use switchboot_updater::{Error, InstallSession, Installation, Updater, UpgradeRequest};

pub struct Updates {
    session: Mutex<Option<InstallSession>>,
    handoff: Option<Result<UpgradeRequest, String>>,
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
            handoff: (!switchboot_core::is_portable())
                .then(|| UpgradeRequest::new().map_err(|error| error.to_string())),
        }
    }
}

impl Updates {
    pub fn shutdown_requested(&self) -> Result<bool, String> {
        match &self.handoff {
            Some(Ok(request)) => request.requested().map_err(|error| error.to_string()),
            Some(Err(error)) => Err(error.clone()),
            None => Ok(false),
        }
    }

    pub fn install(
        &self,
        version: &str,
        progress: impl Fn(u64, Option<u64>),
    ) -> Result<(), String> {
        self.handoff
            .as_ref()
            .ok_or_else(|| Error::Unavailable.to_string())?
            .as_ref()
            .map_err(Clone::clone)?;
        let mut session = self.session.lock().map_err(|error| error.to_string())?;
        if session.is_some() {
            return Err("Windows Setup is already running".into());
        }
        *session = Some(
            Updater::new(env!("CARGO_PKG_VERSION"))
                .and_then(|updater| {
                    updater.install_with_progress(&InstalledWindows, version, progress)
                })
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    }

    pub fn poll(&self) -> Result<bool, String> {
        let mut session = self.session.lock().map_err(|error| error.to_string())?;
        let result = session
            .as_ref()
            .ok_or_else(|| "No Windows Setup process is running".to_owned())?
            .poll()
            .map_err(|error| error.to_string());
        if !matches!(result, Ok(false)) {
            *session = None;
        }
        result
    }
}

struct InstalledWindows;

impl Installation for InstalledWindows {
    fn directory(&self) -> switchboot_updater::Result<PathBuf> {
        if switchboot_core::is_portable() {
            return Err(Error::Unavailable);
        }
        std::env::current_exe()?
            .parent()
            .map(PathBuf::from)
            .ok_or(Error::Unavailable)
    }
}

pub fn check() -> Result<Option<switchboot_ui::UpdateRelease>, String> {
    Updater::new(env!("CARGO_PKG_VERSION"))
        .and_then(|updater| updater.check(&InstalledWindows))
        .map(|release| {
            release.map(|release| switchboot_ui::UpdateRelease {
                version: release.version,
                notes: release.notes,
            })
        })
        .map_err(|error| error.to_string())
}
