use serde::{Deserialize, Serialize};
mod cli_args;

pub const APP_IDENTIFIER: &str = "com.switchboot.app";

#[derive(Debug, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub name: String,
    pub entry_id: u16,
    pub reboot: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CliCommand {
    GetBootOrder,
    SetBootOrder(Vec<u16>),
    GetBootNext,
    SetBootNext(u16),
    GetBootEntries,
    SaveBootOrder(Vec<u16>),
    UnsetBootNext,
    GetBootCurrent,
    Unknown,
}
#[derive(Serialize, Deserialize)]
pub struct BootEntry {
    pub id: u16,
    pub description: String,
    pub is_default: bool,
    pub is_bootnext: bool,
    pub is_current: bool,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct CommandResponse {
    pub code: i32,       // 0 for success, 1 for error
    pub message: String, // stdout or error message
}
