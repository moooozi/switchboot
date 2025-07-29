use firmware_variables::{boot, privileges};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct BootEntry {
    pub id: u16,
    pub description: String,
    pub is_default: bool,
    pub is_bootnext: bool,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_boot_order() -> Result<Vec<u16>, String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::get_boot_order().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_boot_order(order: Vec<u16>) -> Result<(), String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::set_boot_order(&order).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_boot_next() -> Result<Option<u16>, String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::get_boot_next().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_boot_next(entry_id: u16) -> Result<(), String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::set_boot_next(entry_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_boot_entries() -> Result<Vec<BootEntry>, String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    let boot_order = boot::get_boot_order().map_err(|e| e.to_string())?;
    let boot_next = boot::get_boot_next().map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for (idx, &entry_id) in boot_order.iter().enumerate() {
        let parsed = boot::get_parsed_boot_entry(entry_id).map_err(|e| e.to_string())?;
        entries.push(BootEntry {
            id: entry_id,
            description: parsed.description,
            is_default: idx == 0,
            is_bootnext: boot_next == Some(entry_id) && idx != 0,
        });
    }
    Ok(entries)
}

#[tauri::command]
fn save_boot_order(new_order: Vec<u16>) -> Result<(), String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::set_boot_order(&new_order).map_err(|e| e.to_string())
        .and_then(|_| {
            // Optionally, you can also set the first entry as boot next
            if let Some(&first_entry) = new_order.first() {
                boot::set_boot_next(first_entry).map_err(|e| e.to_string())
            } else {
                Ok(())
            }
        })
}

#[tauri::command]
fn unset_boot_next(default_entry: u16) -> Result<(), String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::set_boot_next(default_entry).map_err(|e| e.to_string())
}

// Add more commands as needed for your frontend

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_boot_order,
            set_boot_order,
            get_boot_next,
            set_boot_next,
            get_boot_entries,
            save_boot_order,
            unset_boot_next,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
