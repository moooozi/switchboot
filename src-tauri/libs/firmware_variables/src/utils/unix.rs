use std::fs;

#[derive(Debug)]
pub struct UnsupportedFirmware;

impl std::fmt::Display for UnsupportedFirmware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unsupported firmware")
    }
}
impl std::error::Error for UnsupportedFirmware {}

pub fn verify_uefi_firmware() -> Result<(), UnsupportedFirmware> {
    if fs::metadata("/sys/firmware/efi/efivars").is_ok() {
        Ok(())
    } else {
        Err(UnsupportedFirmware)
    }
}

pub fn gle() -> u32 { 0 }
pub fn nt_status_to_dos_error(_nt_status: i32) -> u32 { 0 }
pub fn utf16_null_terminated(_s: &str) -> Vec<u16> { vec![] }
