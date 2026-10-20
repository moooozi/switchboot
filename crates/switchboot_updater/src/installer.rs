//! Setup requests GUI shutdown, then owns file replacement and restart.
use crate::{Error, Result};
use std::path::{Path, PathBuf};

fn parameters(pid: u32) -> String {
    format!("/S /RESTARTAPP /UPGRADE /UPGRADEPID={pid}")
}

pub(crate) struct StagedInstaller {
    directory: tempfile::TempDir,
    installer: PathBuf,
}

impl StagedInstaller {
    pub fn new(bytes: &[u8]) -> Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("switchboot-update-")
            .tempdir()?;
        let installer = directory.path().join("setup.exe");
        std::fs::write(&installer, bytes)?;
        Ok(Self {
            directory,
            installer,
        })
    }

    pub fn launch(self) -> Result<InstallSession> {
        let session = launch(&self.installer, &parameters(std::process::id()))?;
        // Setup may close the app at any point. Its source must survive process exit.
        // Windows temporary-storage cleanup can reclaim it after installation.
        let _ = self.directory.keep();
        Ok(session)
    }
}

/// Nonblocking observation of Setup. This does not signal or close the app.
/// Drop only closes the monitoring handle; it never terminates the installer.
pub struct InstallSession {
    #[cfg(windows)]
    process: std::os::windows::io::OwnedHandle,
}

impl InstallSession {
    /// `false` means Setup is running; `true` means it finished successfully.
    /// After Switchboot accepts the shutdown request, no further polling is needed.
    pub fn poll(&self) -> Result<bool> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::{
                Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
                System::Threading::{GetExitCodeProcess, WaitForSingleObject},
            };
            let handle = HANDLE(self.process.as_raw_handle());
            match unsafe { WaitForSingleObject(handle, 0) } {
                WAIT_TIMEOUT => Ok(false),
                WAIT_OBJECT_0 => {
                    let mut code = 0;
                    unsafe { GetExitCodeProcess(handle, &mut code) }
                        .map_err(|error| Error::Setup(error.to_string()))?;
                    completed(code)
                }
                _ => Err(Error::Setup("unable to monitor Setup".into())),
            }
        }
        #[cfg(not(windows))]
        Err(Error::Unavailable)
    }
}

#[cfg(any(windows, test))]
fn completed(code: u32) -> Result<bool> {
    if code == 0 {
        Ok(true)
    } else {
        Err(Error::Setup(format!(
            "Setup was canceled or failed (exit code {code})."
        )))
    }
}

#[cfg(not(windows))]
fn launch(_installer: &Path, _parameters: &str) -> Result<InstallSession> {
    Err(Error::Unavailable)
}

#[cfg(windows)]
fn launch(installer: &Path, parameters: &str) -> Result<InstallSession> {
    use std::os::windows::{
        ffi::OsStrExt,
        io::{FromRawHandle, OwnedHandle},
    };
    use windows::{
        Win32::UI::{
            Shell::{
                SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
        core::PCWSTR,
    };
    let path: Vec<u16> = installer.as_os_str().encode_wide().chain(Some(0)).collect();
    let args: Vec<u16> = parameters.encode_utf16().chain(Some(0)).collect();
    // Start Setup unelevated; its worker requests permission for installation.
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(path.as_ptr()),
        lpParameters: PCWSTR(args.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(|error| Error::Setup(error.to_string()))?;
    if info.hProcess.is_invalid() {
        return Err(Error::Setup("unable to monitor Setup".into()));
    }
    // ShellExecuteEx transfers ownership of this handle to the caller.
    Ok(InstallSession {
        process: unsafe { OwnedHandle::from_raw_handle(info.hProcess.0) },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_launch_targets_the_gui_handoff_without_legacy_switches() {
        let args = parameters(1234);
        assert_eq!(args, "/S /RESTARTAPP /UPGRADE /UPGRADEPID=1234");
    }

    #[test]
    fn unused_staging_is_removed_without_launching_setup() {
        let staged = StagedInstaller::new(b"harmless fixture").unwrap();
        let directory = staged.directory.path().to_owned();
        assert_eq!(
            std::fs::read(&staged.installer).unwrap(),
            b"harmless fixture"
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        drop(staged);
        assert!(!directory.exists());
    }

    #[test]
    fn canceled_and_failed_installers_allow_retry() {
        assert!(completed(0).unwrap());
        for code in [1, 2, 5, 1602] {
            assert!(completed(code).is_err());
        }
    }
}
