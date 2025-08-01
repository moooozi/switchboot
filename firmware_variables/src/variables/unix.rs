use super::Attributes;
use crate::utils::verify_uefi_firmware;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

fn efivar_path(name: &str, namespace: &str) -> PathBuf {
    PathBuf::from(format!(
        "/sys/firmware/efi/efivars/{}-{}",
        name,
        namespace.trim_matches(|c| c == '{' || c == '}')
    ))
}

pub fn set_variable(
    name: &str,
    value: &[u8],
    namespace: &str,
    attributes: Attributes,
) -> Result<(), std::io::Error> {
    verify_uefi_firmware()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Unsupported firmware"))?;
    let mut data = attributes.bits().to_le_bytes().to_vec();
    data.extend_from_slice(value);
    let path = efivar_path(name, namespace);
    let mut file = OpenOptions::new().write(true).create(true).open(&path)?;
    file.write_all(&data)?;
    Ok(())
}

pub fn delete_variable(
    name: &str,
    namespace: &str,
    _attributes: Attributes,
) -> Result<(), std::io::Error> {
    let path = efivar_path(name, namespace);
    fs::remove_file(path)
}

pub fn get_variable(name: &str, namespace: &str) -> Result<(Vec<u8>, Attributes), std::io::Error> {
    verify_uefi_firmware()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Unsupported firmware"))?;
    let path = efivar_path(name, namespace);
    let mut file = fs::File::open(&path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    if data.len() < 4 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "efivar file too short",
        ));
    }
    let attributes = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    Ok((
        data[4..].to_vec(),
        Attributes::from_bits_truncate(attributes),
    ))
}
