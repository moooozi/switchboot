use serde::{Deserialize, Serialize};
mod cli_args;

#[derive(Debug, Serialize, Deserialize)]
pub enum ShortcutAction {
    SetBootNext,
    SetFirmwareSetup,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub name: String,
    pub action: ShortcutAction,
    pub entry_id: Option<u16>,
    pub reboot: bool,
    pub icon_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CliCommand {
    GetBootOrder,
    SetBootOrder(Vec<u16>),
    GetBootNext,
    SetBootNext(u16),
    GetBootEntries,
    DiscoverEntries,
    SaveBootOrder(Vec<u16>),
    UnsetBootNext,
    GetBootCurrent,
    SetBootFirmware,
    UnsetBootFirmware,
    GetBootFirmware,
    Unknown,
}
#[derive(Serialize, Deserialize)]
pub struct BootEntry {
    pub id: u16,
    pub description: String,
    pub is_default: Option<bool>,
    pub is_bootnext: bool,
    pub is_current: bool,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct CommandResponse {
    pub code: i32,       // 0 for success, 1 for error
    pub message: String, // stdout or error message
}
