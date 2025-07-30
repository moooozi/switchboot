use firmware_variables::{boot, privileges};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct BootEntry {
    pub id: u16,
    pub description: String,
    pub is_default: bool,
    pub is_bootnext: bool,
    pub is_current: bool,
}


/// Internal logic for setting boot order, used by both Tauri and CLI.
pub fn set_boot_order_internal(order: Vec<u16>) -> Result<(), String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::set_boot_order(&order).map_err(|e| e.to_string())
}

pub fn set_boot_next_internal(entry_id: u16) -> Result<(), String> {
    eprintln!("[DEBUG] set_boot_next_internal called with entry_id: {}", entry_id);
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    let result = boot::set_boot_next(entry_id);
    eprintln!("[DEBUG] boot::set_boot_next returned: {:?}", result);
    result.map_err(|e| e.to_string())
}

pub fn save_boot_order_internal(new_order: Vec<u16>) -> Result<(), String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::set_boot_order(&new_order).map_err(|e| e.to_string())
        .and_then(|_| {
            if let Some(&first_entry) = new_order.first() {
                boot::set_boot_next(first_entry).map_err(|e| e.to_string())
            } else {
                Ok(())
            }
        })
}
pub fn unset_boot_next_internal() -> Result<(), String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::unset_boot_next().map_err(|e| e.to_string())
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
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let args: Vec<String> = order.iter().map(|id| id.to_string()).collect();
        let status = Command::new("pkexec")
            .arg(std::env::current_exe().unwrap())
            .arg("--cli")
            .arg("set-boot-order")
            .args(&args)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Failed to set boot order".into())
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        set_boot_order_internal(order)
    }
}

#[tauri::command]
fn get_boot_next() -> Result<Option<u16>, String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::get_boot_next().map_err(|e| e.to_string())
}


#[tauri::command]
fn get_boot_entries() -> Result<Vec<BootEntry>, String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    let boot_order = boot::get_boot_order().map_err(|e| e.to_string())?;
    let boot_next = boot::get_boot_next().map_err(|e| e.to_string())?;
    let boot_current = boot::get_boot_current().map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for (idx, &entry_id) in boot_order.iter().enumerate() {
        let parsed = boot::get_parsed_boot_entry(entry_id).map_err(|e| e.to_string())?;
        entries.push(BootEntry {
            id: entry_id,
            description: parsed.description,
            is_default: idx == 0,
            is_bootnext: boot_next == Some(entry_id) && idx != 0,
            is_current: boot_current == Some(entry_id),
        });
    }
    Ok(entries)
}

#[tauri::command]
fn set_boot_next(entry_id: u16) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        eprintln!("[DEBUG] set_boot_next: invoking pkexec with entry_id: {}", entry_id);
        let exe = std::env::current_exe().unwrap();
        eprintln!("[DEBUG] Executable path: {:?}", exe);
        let status = Command::new("pkexec")
            .arg(&exe)
            .arg("--cli")
            .arg("set-boot-next")
            .arg(entry_id.to_string())
            .status()
            .map_err(|e| e.to_string())?;
        eprintln!("[DEBUG] pkexec status: {:?}", status);
        if status.success() {
            Ok(())
        } else {
            Err("Failed to set boot next".into())
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        set_boot_next_internal(entry_id)
    }
}

#[tauri::command]
fn save_boot_order(new_order: Vec<u16>) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let args: Vec<String> = new_order.iter().map(|id| id.to_string()).collect();
        let status = Command::new("pkexec")
            .arg(std::env::current_exe().unwrap())
            .arg("--cli")
            .arg("save-boot-order")
            .args(&args)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Failed to save boot order".into())
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        save_boot_order_internal(new_order)
    }
}



#[tauri::command]
fn unset_boot_next() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let status = Command::new("pkexec")
            .arg(std::env::current_exe().unwrap())
            .arg("--cli")
            .arg("unset-boot-next")
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Failed to unset boot next".into())
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        unset_boot_next_internal()
    }
}

#[tauri::command]
fn get_boot_current() -> Result<Option<u16>, String> {
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    boot::get_boot_current().map_err(|e| e.to_string())
}

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
            get_boot_current,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
