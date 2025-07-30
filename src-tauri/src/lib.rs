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

/// with_privileges currently only needed for Windows. Adjusts privileges and runs the provided closure, mapping errors to String.
fn with_privileges<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, Box<dyn std::error::Error>>,
{
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    f().map_err(|e| e.to_string())
}

/// Sets the boot order internally.
pub fn set_boot_order_internal(order: Vec<u16>) -> Result<(), String> {
    with_privileges(|| boot::set_boot_order(&order))
}

/// Sets the BootNext variable internally.
pub fn set_boot_next_internal(entry_id: u16) -> Result<(), String> {
    eprintln!("[DEBUG] set_boot_next_internal called with entry_id: {}", entry_id);
    with_privileges(|| boot::set_boot_next(entry_id))
}

/// Saves the boot order and sets BootNext to the first entry.
pub fn save_boot_order_internal(new_order: Vec<u16>) -> Result<(), String> {
    with_privileges(|| {
        boot::set_boot_order(&new_order)?;
        if let Some(&first_entry) = new_order.first() {
            boot::set_boot_next(first_entry)?;
        }
        Ok(())
    })
}

/// Unsets the BootNext variable internally.
pub fn unset_boot_next_internal() -> Result<(), String> {
    with_privileges(boot::unset_boot_next)
}

#[tauri::command]
fn get_boot_order() -> Result<Vec<u16>, String> {
    with_privileges(boot::get_boot_order)
}

#[cfg(target_os = "linux")]
fn run_with_pkexec(args: &[String]) -> Result<(), String> {
    use std::process::Command;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd_args = vec![String::from("--cli")];
    cmd_args.extend_from_slice(args);
    let status = Command::new("pkexec")
        .arg(exe)
        .args(&cmd_args)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("pkexec command failed".into())
    }
}

#[tauri::command]
fn set_boot_order(order: Vec<u16>) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut args = vec![String::from("set-boot-order")];
        args.extend(order.iter().map(u16::to_string));
        run_with_pkexec(&args)
    }
    #[cfg(not(target_os = "linux"))]
    {
        set_boot_order_internal(order)
    }
}

#[tauri::command]
fn get_boot_next() -> Result<Option<u16>, String> {
    with_privileges(boot::get_boot_next)
}

#[tauri::command]
fn get_boot_entries() -> Result<Vec<BootEntry>, String> {
    with_privileges(|| {
        let boot_order = boot::get_boot_order()?;
        let boot_next = boot::get_boot_next()?;
        let boot_current = boot::get_boot_current()?;
        let mut entries = Vec::new();
        for (idx, &entry_id) in boot_order.iter().enumerate() {
            let parsed = boot::get_parsed_boot_entry(entry_id)?;
            entries.push(BootEntry {
                id: entry_id,
                description: parsed.description,
                is_default: idx == 0,
                is_bootnext: boot_next == Some(entry_id) && idx != 0,
                is_current: boot_current == Some(entry_id),
            });
        }
        Ok(entries)
    })
}

#[tauri::command]
fn set_boot_next(entry_id: u16) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let args = vec![
            String::from("set-boot-next"),
            entry_id.to_string(),
        ];
        run_with_pkexec(&args)
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
        let mut args = vec![String::from("save-boot-order")];
        args.extend(new_order.iter().map(u16::to_string));
        run_with_pkexec(&args)
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
        let args = vec![String::from("unset-boot-next")];
        run_with_pkexec(&args)
    }
    #[cfg(not(target_os = "linux"))]
    {
        unset_boot_next_internal()
    }
}

#[tauri::command]
fn get_boot_current() -> Result<Option<u16>, String> {
    with_privileges(boot::get_boot_current)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
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
