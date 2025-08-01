use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandResponse {
    pub code: i32,       // 0 for success, 1 for error
    pub message: String, // stdout or error message
}

#[derive(Serialize, Deserialize)]
pub struct BootEntry {
    pub id: u16,
    pub description: String,
    pub is_default: bool,
    pub is_bootnext: bool,
    pub is_current: bool,
}
