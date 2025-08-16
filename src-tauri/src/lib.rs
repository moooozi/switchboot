use std::process::Command;
mod cli_user;
pub mod config;
pub mod types;
#[cfg(target_os = "linux")]
use cli_user::call_cli;
#[cfg(target_os = "windows")]
use cli_user::get_cli;
#[cfg(target_os = "windows")]
pub mod windows;

pub use types::{BootEntry, CliCommand, CommandResponse, ShortcutConfig, APP_IDENTIFIER};

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
fn create_shortcut(config: ShortcutConfig) -> Result<(), String> {
    crate::windows::create_shortcut_on_desktop(
        &std::env::current_exe().map_err(|e| e.to_string())?,
        config.entry_id,
        config.reboot,
        &config.name,
        config.icon_id.clone(),
    )
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn create_shortcut(config: ShortcutConfig) -> Result<(), String> {
    use std::env;
    use std::fs;

    // Get the path to the current executable
    let exe = env::current_exe().map_err(|e| e.to_string())?;

    // Build the command line for the shortcut
    let mut exec_cmd = format!(
        "\"{}\" --exec set-boot-next {}",
        exe.display(),
        config.entry_id
    );
    if config.reboot {
        exec_cmd.push_str(" reboot");
    }

    // Build the .desktop file content
    let icon_name = config.icon_id.as_deref().unwrap_or("generic");
    let desktop_entry = format!(
        "[Desktop Entry]\n\
        Type=Application\n\
        Name={}\n\
        Exec={}\n\
        Icon=/usr/share/switchboot/icons/svg/{}.svg\n\
        Terminal=false\n\
        Categories=Utility;\n",
        config.name, exec_cmd, icon_name
    );

    // Get XDG_DATA_HOME or default to ~/.local/share
    let data_home = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            println!("XDG_DATA_HOME not set, using default ~/.local/share");
            std::env::var("HOME").ok().map(|home| {
                let mut p = std::path::PathBuf::from(home);
                p.push(".local/share");
                p
            })
        })
        .ok_or_else(|| "Could not determine data directory".to_string())?;

    let mut desktop_path = data_home;
    desktop_path.push("applications");
    fs::create_dir_all(&desktop_path).map_err(|e| e.to_string())?;
    desktop_path.push(format!("{}-{}.desktop", APP_IDENTIFIER, config.entry_id));
    fs::write(&desktop_path, desktop_entry).map_err(|e| e.to_string())?;

    // Make the .desktop file executable
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(&desktop_path)
        .map_err(|e| e.to_string())?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&desktop_path, perms).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn handle_bootnext_shortcut_execution(
    entry_id: u16,
    should_reboot: bool,
) -> Result<(), String> {
    set_boot_next(entry_id)?;
    if should_reboot {
        restart_now()?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn restart_now() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    Command::new("shutdown")
        .args(&["/r", "/t", "0"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
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

#[cfg(target_os = "windows")]
#[tauri::command]
fn is_portable() -> bool {
    config::is_portable_mode()
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn is_portable() -> bool {
    false
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(app_config: Option<config::AppConfig>) {
    // Initialize configuration if provided
    if let Some(cfg) = app_config {
        config::init_config(cfg);
    }

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
            create_shortcut,
            restart_now,
            is_portable,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
