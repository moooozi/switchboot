use serde::{Deserialize, Serialize};
use std::process::Command;
mod cli_client;
use cli_client::{call_cli, get_cli};
mod cli;

#[derive(Serialize, Deserialize)]
pub struct BootEntry {
    pub id: u16,
    pub description: String,
    pub is_default: bool,
    pub is_bootnext: bool,
    pub is_current: bool,
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_order() -> Result<Vec<u16>, String> {
    let out = call_cli(&["get-boot-order"], false)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_order() -> Result<Vec<u16>, String> {
    get_cli()?.send_command(&["get-boot-order"])
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn set_boot_order(order: Vec<u16>) -> Result<(), String> {
    let args: Vec<String> = std::iter::once("set-boot-order".to_string())
        .chain(order.iter().map(u16::to_string))
        .collect();
    call_cli(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>(), true)?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_boot_order(order: Vec<u16>) -> Result<(), String> {
    let args: Vec<String> = std::iter::once("set-boot-order".to_string())
        .chain(order.iter().map(u16::to_string))
        .collect();
    get_cli()?.send_command_unit(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_next() -> Result<Option<u16>, String> {
    let out = call_cli(&["get-boot-next"], false)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_next() -> Result<Option<u16>, String> {
    get_cli()?.send_command(&["get-boot-next"])
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn set_boot_next(entry_id: u16) -> Result<(), String> {
    call_cli(&["set-boot-next", &entry_id.to_string()], true)?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_boot_next(entry_id: u16) -> Result<(), String> {
    get_cli()?.send_command_unit(&["set-boot-next", &entry_id.to_string()])
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn save_boot_order(new_order: Vec<u16>) -> Result<(), String> {
    let args: Vec<String> = std::iter::once("save-boot-order".to_string())
        .chain(new_order.iter().map(u16::to_string))
        .collect();
    call_cli(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>(), true)?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn save_boot_order(new_order: Vec<u16>) -> Result<(), String> {
    let args: Vec<String> = std::iter::once("save-boot-order".to_string())
        .chain(new_order.iter().map(u16::to_string))
        .collect();
    get_cli()?.send_command_unit(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn unset_boot_next() -> Result<(), String> {
    call_cli(&["unset-boot-next"], true)?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn unset_boot_next() -> Result<(), String> {
    get_cli()?.send_command_unit(&["unset-boot-next"])
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_entries() -> Result<Vec<BootEntry>, String> {
    let out = call_cli(&["get-boot-entries"], false)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_entries() -> Result<Vec<BootEntry>, String> {
    get_cli()?.send_command(&["get-boot-entries"])
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_current() -> Result<Option<u16>, String> {
    let out = call_cli(&["get-boot-current"], false)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_current() -> Result<Option<u16>, String> {
    get_cli()?.send_command(&["get-boot-current"])
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn restart_now() -> Result<(), String> {
    Command::new("shutdown")
        .args(&["/r", "/t", "0"])
        .spawn()
        .map_err(|e| format!("Failed to execute shutdown: {e}"))?;
    Ok(())
}

#[cfg(unix)]
#[tauri::command]
fn restart_now() -> Result<(), String> {
    let shutdown_result = Command::new("shutdown").args(&["-r", "now"]).spawn();
    if shutdown_result.is_err() {
        Command::new("reboot")
            .spawn()
            .map_err(|e| format!("Failed to execute reboot: {e}"))?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
#[tauri::command]
fn restart_now() -> Result<(), String> {
    Err("Unsupported platform".to_string())
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
            restart_now,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
