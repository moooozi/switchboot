use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{GetLastError, ERROR_INVALID_FUNCTION, RtlNtStatusToDosError, NTSTATUS};
use windows::Win32::System::WindowsProgramming::GetFirmwareEnvironmentVariableExW;

#[derive(Debug)]
pub struct UnsupportedFirmware;

impl std::fmt::Display for UnsupportedFirmware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unsupported firmware")
    }
}
impl std::error::Error for UnsupportedFirmware {}

pub fn verify_uefi_firmware() -> Result<(), UnsupportedFirmware> {
    let mut attributes: u32 = 0;
    // Empty buffer, as in Python
    let mut buffer = [];
    let name = utf16_null_terminated("");
    let guid = utf16_null_terminated("{00000000-0000-0000-0000-000000000000}");

    let stored_bytes = unsafe {
        GetFirmwareEnvironmentVariableExW(
            PCWSTR(name.as_ptr()),
            PCWSTR(guid.as_ptr()),
            Some(buffer.as_mut_ptr() as *mut _),
            buffer.len() as u32,
            Some(&mut attributes as *mut u32),
        )
    };

    if stored_bytes == 0 && gle() == ERROR_INVALID_FUNCTION.0 {
        Err(UnsupportedFirmware)
    } else {
        Ok(())
    }
}

pub fn gle() -> u32 {
    unsafe { GetLastError().0 }
}

pub fn nt_status_to_dos_error(nt_status: NTSTATUS) -> u32 {
    unsafe { RtlNtStatusToDosError(nt_status) }
}

pub fn utf16_null_terminated(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}
