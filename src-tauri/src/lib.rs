use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct BootEntry {
    pub id: u16,
    pub description: String,
    pub is_default: bool,
    pub is_bootnext: bool,
    pub is_current: bool,
}

fn call_cli(args: &[&str]) -> Result<String, String> {
    let cli_path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|p| p.join("switchboot-cli"))
        .ok_or("Failed to find CLI binary")?;

    // List of commands that require privilege
    let needs_privilege = matches!(
        args.get(0).map(|s| *s),
        Some("set-boot-order")
            | Some("set-boot-next")
            | Some("save-boot-order")
            | Some("unset-boot-next")
    );

    let mut cmd = if needs_privilege {
        let mut c = Command::new("pkexec");
        c.arg(&cli_path);
        c
    } else {
        Command::new(&cli_path)
    };

    cmd.args(args);

    let output = cmd.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
fn get_boot_order() -> Result<Vec<u16>, String> {
    let out = call_cli(&["get-boot-order"])?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_boot_order(order: Vec<u16>) -> Result<(), String> {
    let args: Vec<String> = std::iter::once("set-boot-order".to_string())
        .chain(order.iter().map(u16::to_string))
        .collect();
    call_cli(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;
    Ok(())
}

#[tauri::command]
fn get_boot_next() -> Result<Option<u16>, String> {
    let out = call_cli(&["get-boot-next"])?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_boot_next(entry_id: u16) -> Result<(), String> {
    call_cli(&["set-boot-next", &entry_id.to_string()])?;
    Ok(())
}

#[tauri::command]
fn save_boot_order(new_order: Vec<u16>) -> Result<(), String> {
    let args: Vec<String> = std::iter::once("save-boot-order".to_string())
        .chain(new_order.iter().map(u16::to_string))
        .collect();
    call_cli(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;
    Ok(())
}

#[tauri::command]
fn unset_boot_next() -> Result<(), String> {
    call_cli(&["unset-boot-next"])?;
    Ok(())
}

#[tauri::command]
fn get_boot_entries() -> Result<Vec<BootEntry>, String> {
    let out = call_cli(&["get-boot-entries"])?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_boot_current() -> Result<Option<u16>, String> {
    let out = call_cli(&["get-boot-current"])?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
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
