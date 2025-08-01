use crate::device_path::DevicePathList;
use crate::utils::{string_to_utf16_bytes, utf16_string_from_bytes};
use bitflags::bitflags;

#[repr(C)]
pub struct LoadOption {
    pub attributes: LoadOptionAttributes,
    pub description: String,
    pub file_path_list: DevicePathList,
    pub optional_data: Vec<u8>,
}

bitflags! {
    #[derive(Debug)]
    pub struct LoadOptionAttributes: u32 {
        const LOAD_OPTION_ACTIVE = 0x00000001;
        const LOAD_OPTION_FORCE_RECONNECT = 0x00000002;
        const LOAD_OPTION_HIDDEN = 0x00000008;
        const LOAD_OPTION_CATEGORY_APP = 0x00000100;
    }
}

impl LoadOption {
    pub fn from_bytes(raw: &[u8]) -> Option<Self> {
        if raw.len() < 6 {
            return None;
        }
        // EFI_LOAD_OPTION: <IH (u32, u16)
        let attributes = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        let file_path_list_length = u16::from_le_bytes([raw[4], raw[5]]) as usize;

        // Decode description (null-terminated UTF-16LE string)
        let desc_start = 6;
        let desc = match utf16_string_from_bytes(&raw[desc_start..]) {
            Ok(s) => s,
            Err(_) => {
                // Fallback: try to decode as UTF-16LE lossy, or use placeholder
                let mut lossy = String::new();
                let mut i = 0;
                while i + 1 < raw[desc_start..].len() {
                    let u = u16::from_le_bytes([raw[desc_start + i], raw[desc_start + i + 1]]);
                    if u == 0 { break; }
                    lossy.push(std::char::from_u32(u as u32).unwrap_or('?'));
                    i += 2;
                }
                if lossy.is_empty() {
                    "<invalid description>".to_string()
                } else {
                    lossy
                }
            }
        };

        // Calculate offset for file_path_list
        let str_size = (desc.len() + 1) * 2;
        let file_path_list_offset = desc_start + str_size;
        // If the file path list is truncated or malformed, just use what is available
        let file_path_list_bytes = if file_path_list_offset >= raw.len() {
            &[]
        } else if file_path_list_offset + file_path_list_length > raw.len() {
            &raw[file_path_list_offset..]
        } else {
            &raw[file_path_list_offset..file_path_list_offset + file_path_list_length]
        };
        let file_path_list = DevicePathList::from_bytes(file_path_list_bytes);

        // Optional data
        let optional_data = if file_path_list_offset + file_path_list_length > raw.len() {
            Vec::new()
        } else {
            raw[file_path_list_offset + file_path_list_length..].to_vec()
        };

        Some(Self {
            attributes: LoadOptionAttributes::from_bits_truncate(attributes),
            description: desc,
            file_path_list,
            optional_data,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let raw_file_path_list = self.file_path_list.to_bytes();
        let mut raw = Vec::with_capacity(
            6 + self.description.len() * 2 + raw_file_path_list.len() + self.optional_data.len(),
        );

        // Header: attributes (u32), file_path_list_length (u16)
        raw.extend(&self.attributes.bits().to_le_bytes());
        raw.extend(&(raw_file_path_list.len() as u16).to_le_bytes());

        // Description (UTF-16LE null-terminated)
        raw.extend(string_to_utf16_bytes(&self.description));

        // File path list
        raw.extend(&raw_file_path_list);

        // Optional data
        raw.extend(&self.optional_data);

        raw
    }
}

impl std::fmt::Debug for LoadOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<{} {:?} [{:?}]>",
            self.description, self.file_path_list, self.attributes
        )
    }
}
