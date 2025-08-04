use std::process::Command;
mod cli_user;
pub mod types;
#[cfg(target_os = "linux")]
use cli_user::call_cli;
#[cfg(target_os = "windows")]
use cli_user::get_cli;
#[cfg(target_os = "windows")]
pub mod windows;

pub use types::{BootEntry, CliCommand, CommandResponse};

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_order() -> Result<Vec<u16>, String> {
    let out = call_cli(&CliCommand::GetBootOrder, false)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_order() -> Result<Vec<u16>, String> {
    get_cli()?.send_command(&CliCommand::GetBootOrder)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn set_boot_order(order: Vec<u16>) -> Result<(), String> {
    call_cli(&CliCommand::SetBootOrder(order), true)?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_boot_order(order: Vec<u16>) -> Result<(), String> {
    get_cli()?.send_command_unit(&CliCommand::SetBootOrder(order))
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_next() -> Result<Option<u16>, String> {
    let out = call_cli(&CliCommand::GetBootNext, false)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_next() -> Result<Option<u16>, String> {
    get_cli()?.send_command(&CliCommand::GetBootNext)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn set_boot_next(entry_id: u16) -> Result<(), String> {
    call_cli(&CliCommand::SetBootNext(entry_id), true)?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_boot_next(entry_id: u16) -> Result<(), String> {
    get_cli()?.send_command_unit(&CliCommand::SetBootNext(entry_id))
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn save_boot_order(new_order: Vec<u16>) -> Result<(), String> {
    call_cli(&CliCommand::SaveBootOrder(new_order), true)?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn save_boot_order(new_order: Vec<u16>) -> Result<(), String> {
    get_cli()?.send_command_unit(&CliCommand::SaveBootOrder(new_order))
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn unset_boot_next() -> Result<(), String> {
    call_cli(&CliCommand::UnsetBootNext, true)?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn unset_boot_next() -> Result<(), String> {
    get_cli()?.send_command_unit(&CliCommand::UnsetBootNext)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_entries() -> Result<Vec<BootEntry>, String> {
    let out = call_cli(&CliCommand::GetBootEntries, false)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_entries() -> Result<Vec<BootEntry>, String> {
    get_cli()?.send_command(&CliCommand::GetBootEntries)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_current() -> Result<Option<u16>, String> {
    let out = call_cli(&CliCommand::GetBootCurrent, false)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_current() -> Result<Option<u16>, String> {
    get_cli()?.send_command(&CliCommand::GetBootCurrent)
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
