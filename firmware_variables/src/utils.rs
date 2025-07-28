use std::ffi::OsStr;
use std::iter::once;
use std::mem;
use std::os::windows::ffi::OsStrExt;

use windows::core::PCWSTR;
use windows::Win32::Foundation::RtlNtStatusToDosError;
use windows::Win32::Foundation::NTSTATUS;
use windows::Win32::Foundation::{GetLastError, ERROR_INVALID_FUNCTION};
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

pub fn utf16_string_from_bytes(raw: &[u8]) -> Result<String, &'static str> {
    if raw.len() % 2 != 0 {
        return Err("Input length is not even");
    }
    let mut end = 0;
    while end + 1 < raw.len() {
        if raw[end] == 0 && raw[end + 1] == 0 {
            break;
        }
        end += 2;
    }
    String::from_utf16(
        &raw[..end]
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| "Invalid UTF-16")
}

pub fn string_to_utf16_bytes(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    v.extend_from_slice(&[0, 0]);
    v
}

pub fn iter_unpack<'a, T: Copy>(buffer: &'a [u8]) -> impl Iterator<Item = T> + 'a {
    buffer.chunks_exact(mem::size_of::<T>()).map(|chunk| {
        let size = mem::size_of::<T>();
        assert_eq!(chunk.len(), size);
        let mut t = std::mem::MaybeUninit::<T>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(chunk.as_ptr(), t.as_mut_ptr() as *mut u8, size);
            t.assume_init()
        }
    })
}

// Helper: Convert &str to null-terminated UTF-16 Vec<u16>
fn utf16_null_terminated(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}
