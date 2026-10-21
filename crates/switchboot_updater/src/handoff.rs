//! One per-GUI event. Setup signals it after elevation and preflight.
use crate::{Error, Result};

pub struct UpgradeRequest {
    #[cfg(windows)]
    event: std::os::windows::io::OwnedHandle,
}

#[cfg(windows)]
fn event_name(pid: u32) -> String {
    format!(r"Local\Switchboot.Upgrade.{pid}")
}

impl UpgradeRequest {
    pub fn new() -> Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::io::{FromRawHandle, OwnedHandle};
            use windows::{
                Win32::{
                    Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
                    System::Threading::CreateEventW,
                },
                core::PCWSTR,
            };
            let name: Vec<u16> = event_name(std::process::id())
                .encode_utf16()
                .chain(Some(0))
                .collect();
            // The default security descriptor limits access to the creating
            // user's token DACL; handles are not inherited by CLI children.
            let handle = unsafe { CreateEventW(None, false, false, PCWSTR(name.as_ptr())) }
                .map_err(|error| Error::Setup(error.to_string()))?;
            let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
            let event = unsafe { OwnedHandle::from_raw_handle(handle.0) };
            if already_exists {
                return Err(Error::Setup("upgrade event already exists".into()));
            }
            Ok(Self { event })
        }
        #[cfg(not(windows))]
        Err(Error::Unavailable)
    }

    /// Nonblocking; consumes one shutdown request. Only the GUI calls this.
    pub fn requested(&self) -> Result<bool> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::{
                Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
                System::Threading::WaitForSingleObject,
            };
            match unsafe { WaitForSingleObject(HANDLE(self.event.as_raw_handle()), 0) } {
                WAIT_OBJECT_0 => Ok(true),
                WAIT_TIMEOUT => Ok(false),
                _ => Err(Error::Setup("unable to read upgrade request".into())),
            }
        }
        #[cfg(not(windows))]
        Err(Error::Unavailable)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn request_is_private_per_process_and_consumed_once() {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::{Foundation::HANDLE, System::Threading::SetEvent};
        let request = UpgradeRequest::new().unwrap();
        assert!(!request.requested().unwrap());
        assert!(UpgradeRequest::new().is_err());
        unsafe { SetEvent(HANDLE(request.event.as_raw_handle())) }.unwrap();
        assert!(request.requested().unwrap());
        assert!(!request.requested().unwrap());
    }
}
