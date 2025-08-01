use std::convert::TryFrom;
use std::fmt;
use uuid::Uuid;

use crate::utils::{string_to_utf16_bytes, utf16_string_from_bytes};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevicePathType {
    HardwareDevicePath = 0x01,
    AcpiDevicePath = 0x02,
    MessagingDevicePath = 0x03,
    MediaDevicePath = 0x04,
    BiosBootSpecificationDevicePath = 0x05,
    EndOfHardwareDevicePath = 0x7F,
}

impl TryFrom<u8> for DevicePathType {
    type Error = ();
    fn try_from(v: u8) -> Result<Self, ()> {
        use DevicePathType::*;
        Ok(match v {
            0x01 => HardwareDevicePath,
            0x02 => AcpiDevicePath,
            0x03 => MessagingDevicePath,
            0x04 => MediaDevicePath,
            0x05 => BiosBootSpecificationDevicePath,
            0x7F => EndOfHardwareDevicePath,
            _ => return Err(()),
        })
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaDevicePathSubtype {
    HardDrive = 0x01,
    CdRom = 0x02,
    Vendor = 0x03,
    FilePath = 0x04,
    MediaProtocol = 0x05,
    PiwgFirmwareFile = 0x06,
    PiwgFirmwareVolume = 0x07,
    RelativeOffsetRange = 0x08,
    RamDiskDevicePath = 0x09,
}

impl TryFrom<u8> for MediaDevicePathSubtype {
    type Error = ();
    fn try_from(v: u8) -> Result<Self, ()> {
        use MediaDevicePathSubtype::*;
        Ok(match v {
            0x01 => HardDrive,
            0x02 => CdRom,
            0x03 => Vendor,
            0x04 => FilePath,
            0x05 => MediaProtocol,
            0x06 => PiwgFirmwareFile,
            0x07 => PiwgFirmwareVolume,
            0x08 => RelativeOffsetRange,
            0x09 => RamDiskDevicePath,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct HardDriveNode {
    pub partition_number: u32,
    pub partition_start_lba: u64,
    pub partition_size_lba: u64,
    pub partition_signature: Vec<u8>,
    pub partition_guid: Option<Uuid>,
    pub partition_format: u8,
    pub signature_type: u8,
}

#[derive(Debug, Clone)]
pub struct DevicePath {
    pub path_type: DevicePathType,
    pub subtype: u8,
    pub data: Vec<u8>,
}

impl DevicePath {
    pub fn is_hard_drive(&self) -> bool {
        self.path_type == DevicePathType::MediaDevicePath
            && self.subtype == MediaDevicePathSubtype::HardDrive as u8
    }

    pub fn get_hard_drive_node(&self) -> Option<HardDriveNode> {
        if !self.is_hard_drive() || self.data.len() < 38 {
            return None;
        }
        let partition_number = u32::from_le_bytes(self.data[0..4].try_into().unwrap());
        let partition_start_lba = u64::from_le_bytes(self.data[4..12].try_into().unwrap());
        let partition_size_lba = u64::from_le_bytes(self.data[12..20].try_into().unwrap());
        let signature = self.data[20..36].to_vec();
        let partition_format = self.data[36];
        let signature_type = self.data[37];
        let partition_guid =
            if partition_format == 2 && signature_type == 2 && signature.len() == 16 {
                Some(Uuid::from_bytes_le(
                    signature.as_slice().try_into().unwrap(),
                ))
            } else {
                None
            };
        Some(HardDriveNode {
            partition_number,
            partition_start_lba,
            partition_size_lba,
            partition_signature: signature,
            partition_guid,
            partition_format,
            signature_type,
        })
    }

    pub fn set_hard_drive_node(&mut self, node: &HardDriveNode) -> bool {
        if !self.is_hard_drive() {
            return false;
        }
        let mut data = Vec::with_capacity(38);
        data.extend_from_slice(&node.partition_number.to_le_bytes());
        data.extend_from_slice(&node.partition_start_lba.to_le_bytes());
        data.extend_from_slice(&node.partition_size_lba.to_le_bytes());
        data.extend_from_slice(&node.partition_signature);
        data.push(node.partition_format);
        data.push(node.signature_type);
        self.data = data;
        true
    }

    pub fn is_file_path(&self) -> bool {
        self.path_type == DevicePathType::MediaDevicePath
            && self.subtype == MediaDevicePathSubtype::FilePath as u8
    }

    pub fn get_file_path(&self) -> Option<String> {
        if !self.is_file_path() {
            return None;
        }
        utf16_string_from_bytes(&self.data).ok()
    }

    pub fn set_file_path(&mut self, file_path: &str) -> bool {
        if !self.is_file_path() {
            return false;
        }
        self.data = string_to_utf16_bytes(file_path);
        true
    }
}

pub struct DevicePathList {
    pub paths: Vec<DevicePath>,
}

impl DevicePathList {
    pub fn from_bytes(raw: &[u8]) -> Self {
        let mut paths = Vec::new();
        let mut offset = 0;
        while offset + 4 <= raw.len() {
            let path_type = DevicePathType::try_from(raw[offset])
                .unwrap_or(DevicePathType::EndOfHardwareDevicePath);
            let subtype = raw[offset + 1];
            let length = u16::from_le_bytes([raw[offset + 2], raw[offset + 3]]) as usize;
            if offset + length > raw.len() || length < 4 {
                break;
            }
            let data = raw[offset + 4..offset + length].to_vec();
            paths.push(DevicePath {
                path_type,
                subtype,
                data,
            });
            offset += length;
        }
        DevicePathList { paths }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut raw = Vec::new();
        for path in &self.paths {
            let length = 4 + path.data.len();
            raw.push(path.path_type as u8);
            raw.push(path.subtype);
            raw.extend_from_slice(&(length as u16).to_le_bytes());
            raw.extend_from_slice(&path.data);
        }
        raw
    }

    pub fn get_file_path(&self) -> Option<String> {
        for path in &self.paths {
            if let Some(fp) = path.get_file_path() {
                return Some(fp);
            }
        }
        None
    }

    pub fn set_file_path(&mut self, file_path: &str) -> bool {
        for path in &mut self.paths {
            if path.is_file_path() {
                return path.set_file_path(file_path);
            }
        }
        false
    }

    pub fn get_hard_drive_node(&self) -> Option<HardDriveNode> {
        for path in &self.paths {
            if let Some(node) = path.get_hard_drive_node() {
                return Some(node);
            }
        }
        None
    }

    pub fn set_hard_drive_node(&mut self, node: &HardDriveNode) -> bool {
        for path in &mut self.paths {
            if path.is_hard_drive() {
                return path.set_hard_drive_node(node);
            }
        }
        false
    }
}

impl fmt::Debug for DevicePathList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(fp) = self.get_file_path() {
            write!(f, "{fp}")
        } else {
            write!(f, "<Custom Location>")
        }
    }
}
