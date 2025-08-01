use super::Attributes;
use crate::utils::{gle, verify_uefi_firmware};
use windows::core::Error as WinError;
use windows::core::PCWSTR;
use windows::Win32::System::WindowsProgramming::{
    GetFirmwareEnvironmentVariableExW, SetFirmwareEnvironmentVariableExW,
};

pub fn set_variable(
    name: &str,
    value: &[u8],
    namespace: &str,
    attributes: Attributes,
) -> Result<(), windows::core::Error> {
    verify_uefi_firmware().map_err(|_| windows::core::Error::from_win32())?;
    let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let guid_wide: Vec<u16> = namespace.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        SetFirmwareEnvironmentVariableExW(
            PCWSTR(name_wide.as_ptr()),
            PCWSTR(guid_wide.as_ptr()),
            Some(value.as_ptr() as *mut _),
            value.len() as u32,
            attributes.bits(),
        )?
    }
    Ok(())
}

pub fn delete_variable(
    name: &str,
    namespace: &str,
    attributes: Attributes,
) -> Result<(), windows::core::Error> {
    set_variable(name, &[], namespace, attributes)
}

pub fn get_variable(name: &str, namespace: &str) -> Result<(Vec<u8>, Attributes), WinError> {
    verify_uefi_firmware().map_err(|_| WinError::from_win32())?;

    let mut allocation = 16;
    loop {
        let mut attributes: u32 = 0;
        let mut buffer = vec![0u8; allocation];
        let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let guid_wide: Vec<u16> = namespace.encode_utf16().chain(std::iter::once(0)).collect();
        let stored_bytes = unsafe {
            GetFirmwareEnvironmentVariableExW(
                PCWSTR(name_wide.as_ptr()),
                PCWSTR(guid_wide.as_ptr()),
                Some(buffer.as_mut_ptr() as *mut _),
                buffer.len() as u32,
                Some(&mut attributes as *mut u32),
            )
        };
        if stored_bytes != 0 {
            buffer.truncate(stored_bytes as usize);
            return Ok((buffer, Attributes::from_bits_truncate(attributes)));
        } else if gle() == 122 {
            // ERROR_BUFFER_TOO_SMALL
            allocation *= 2;
        } else {
            return Err(WinError::from_win32());
        }
    }
}
