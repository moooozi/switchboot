use crate::load_option::LoadOption;
use crate::utils::{iter_unpack, verify_uefi_firmware};
use crate::variables::{get_variable, set_variable, DEFAULT_ATTRIBUTES, GLOBAL_NAMESPACE};

pub fn get_boot_order() -> Result<Vec<u16>, Box<dyn std::error::Error>> {
    verify_uefi_firmware()?;
    let (raw, _) = get_variable("BootOrder", GLOBAL_NAMESPACE)?;
    // Each entry is a little-endian u16
    let ids: Vec<u16> = iter_unpack::<u16>(&raw).collect();
    Ok(ids)
}

pub fn set_boot_order(entry_ids: &[u16]) -> Result<(), Box<dyn std::error::Error>> {
    verify_uefi_firmware()?;
    let mut raw = Vec::with_capacity(entry_ids.len() * 2);
    for &id in entry_ids {
        raw.extend(&id.to_le_bytes());
    }
    let result = set_variable("BootOrder", &raw, GLOBAL_NAMESPACE, DEFAULT_ATTRIBUTES);
    match result {
        Ok(_) => (),
        Err(ref e) => println!("[boot] set_boot_order failed: {:?}", e),
    }
    result?;
    Ok(())
}

pub fn get_boot_entry(entry_id: u16) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    verify_uefi_firmware()?;
    let name = format!("Boot{:04X}", entry_id);
    let (raw, _) = get_variable(&name, GLOBAL_NAMESPACE)?;
    Ok(raw)
}

pub fn get_parsed_boot_entry(entry_id: u16) -> Result<LoadOption, Box<dyn std::error::Error>> {
    let raw = get_boot_entry(entry_id)?;
    LoadOption::from_bytes(&raw).ok_or_else(|| "Failed to parse LoadOption".into())
}

pub fn set_boot_entry(entry_id: u16, raw: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    verify_uefi_firmware()?;
    let name = format!("Boot{:04X}", entry_id);
    set_variable(&name, raw, GLOBAL_NAMESPACE, DEFAULT_ATTRIBUTES)?;
    Ok(())
}

pub fn set_parsed_boot_entry(
    entry_id: u16,
    load_option: &LoadOption,
) -> Result<(), Box<dyn std::error::Error>> {
    set_boot_entry(entry_id, &load_option.to_bytes())
}

pub fn get_boot_next() -> Result<Option<u16>, Box<dyn std::error::Error>> {
    verify_uefi_firmware()?;
    match get_variable("BootNext", GLOBAL_NAMESPACE) {
        Ok((raw, _)) if raw.len() >= 2 => {
            let val = u16::from_le_bytes([raw[0], raw[1]]);
            Ok(Some(val))
        }
        _ => Ok(None),
    }
}

pub fn set_boot_next(entry_id: u16) -> Result<(), Box<dyn std::error::Error>> {
    verify_uefi_firmware()?;
    let raw = entry_id.to_le_bytes();
    set_variable("BootNext", &raw, GLOBAL_NAMESPACE, DEFAULT_ATTRIBUTES)?;
    Ok(())
}

pub fn unset_boot_next() -> Result<(), Box<dyn std::error::Error>> {
    verify_uefi_firmware()?;
    match crate::variables::delete_variable("BootNext", GLOBAL_NAMESPACE, DEFAULT_ATTRIBUTES) {
        Ok(_) => Ok(()),
        #[cfg(unix)]
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()), // Already unset
        Err(e) => Err(Box::new(e)),
    }
}

/// Returns the Boot#### entry ID used to boot the current session, if available.
pub fn get_boot_current() -> Result<Option<u16>, Box<dyn std::error::Error>> {
    verify_uefi_firmware()?;
    match get_variable("BootCurrent", GLOBAL_NAMESPACE) {
        Ok((raw, _)) if raw.len() >= 2 => {
            let val = u16::from_le_bytes([raw[0], raw[1]]);
            Ok(Some(val))
        }
        _ => Ok(None),
    }
}
