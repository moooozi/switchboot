use std::process::Command;
pub mod build_info;
pub mod cli;
mod cli_user;
pub mod constants;
pub mod types;
#[cfg(target_os = "linux")]
use cli_user::call_cli;
#[cfg(target_os = "windows")]
use cli_user::get_cli;
#[cfg(target_os = "windows")]
pub mod windows;

use tauri::Manager;
pub use types::{BootEntry, CliCommand, CommandResponse, ShortcutAction, ShortcutConfig};
// Re-export build metadata from top-level module
pub use build_info::APP_IDENTIFIER;

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_order() -> Result<Vec<u16>, String> {
    let out = call_cli(&CliCommand::GetBootOrder)?;
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
    call_cli(&CliCommand::SetBootOrder(order))?;
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
    let out = call_cli(&CliCommand::GetBootNext)?;
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
    call_cli(&CliCommand::SetBootNext(entry_id))?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_boot_next(entry_id: u16) -> Result<(), String> {
    get_cli()?.send_command_unit(&CliCommand::SetBootNext(entry_id))
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn unset_boot_next() -> Result<(), String> {
    call_cli(&CliCommand::UnsetBootNext)?;
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
    let out = call_cli(&CliCommand::GetBootEntries)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_entries() -> Result<Vec<BootEntry>, String> {
    get_cli()?.send_command(&CliCommand::GetBootEntries)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
async fn discover_entries() -> Result<Vec<BootEntry>, String> {
    let out = call_cli(&CliCommand::DiscoverEntries)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn discover_entries() -> Result<Vec<BootEntry>, String> {
    get_cli()?.send_command(&CliCommand::DiscoverEntries)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_current() -> Result<Option<u16>, String> {
    let out = call_cli(&CliCommand::GetBootCurrent)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_current() -> Result<Option<u16>, String> {
    get_cli()?.send_command(&CliCommand::GetBootCurrent)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn set_boot_fw() -> Result<(), String> {
    call_cli(&CliCommand::SetBootFirmware)?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_boot_fw() -> Result<(), String> {
    get_cli()?.send_command_unit(&CliCommand::SetBootFirmware)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn unset_boot_fw() -> Result<(), String> {
    call_cli(&CliCommand::UnsetBootFirmware)?;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn unset_boot_fw() -> Result<(), String> {
    get_cli()?.send_command_unit(&CliCommand::UnsetBootFirmware)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_boot_fw() -> Result<bool, String> {
    let out = call_cli(&CliCommand::GetBootFirmware)?;
    serde_json::from_str(&out).map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_boot_fw() -> Result<bool, String> {
    get_cli()?.send_command(&CliCommand::GetBootFirmware)
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn create_shortcut(config: ShortcutConfig) -> Result<(), String> {
    crate::windows::create_shortcut_on_desktop(
        &std::env::current_exe().map_err(|e| e.to_string())?,
        &config.action,
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
    let mut exec_cmd = match config.action {
        ShortcutAction::SetBootNext => {
            if let Some(id) = config.entry_id {
                format!("\"{}\" --exec set-boot-next {}", exe.display(), id)
            } else {
                return Err("entry_id required for SetBootNext action".to_string());
            }
        }
        ShortcutAction::SetFirmwareSetup => format!("\"{}\" --exec set-boot-fw", exe.display()),
    };
    if config.reboot {
        exec_cmd.push_str(" reboot");
    }

    // Build the .desktop file content
    let icon_name = config.icon_id.as_deref().unwrap_or("generic");
    let sanitized_name = config
        .name
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    let desktop_entry = format!(
        "[Desktop Entry]\n\
        Type=Application\n\
        Name={}\n\
        Exec={}\n\
        Icon=swboot-{}\n\
        Terminal=false\n\
        Categories=Utility;\n",
        sanitized_name, exec_cmd, icon_name
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
    // Choose the filename extension based on whether this shortcut reboots
    let name_extension = if config.reboot { "reboot" } else { "bootnext" };

    // Build the final desktop file path without mutating the applications directory path
    let desktop_file = match config.action {
        ShortcutAction::SetBootNext => {
            if let Some(id) = config.entry_id {
                desktop_path.join(format!(
                    "{}-{}-{}.desktop",
                    APP_IDENTIFIER, name_extension, id
                ))
            } else {
                return Err("entry_id required for SetBootNext action".to_string());
            }
        }
        ShortcutAction::SetFirmwareSetup => desktop_path.join(format!(
            "{}-{}-firmware.desktop",
            APP_IDENTIFIER, name_extension
        )),
    };

    fs::write(&desktop_file, desktop_entry).map_err(|e| e.to_string())?;

    // Make the .desktop file executable
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(&desktop_file)
        .map_err(|e| e.to_string())?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&desktop_file, perms).map_err(|e| e.to_string())?;

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

pub fn handle_bootfw_shortcut_execution(should_reboot: bool) -> Result<(), String> {
    set_boot_fw()?;
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
    windows::is_portable_mode()
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn is_portable() -> bool {
    false
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(_app_config: Option<()>) {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Single instance plugin to focus existing window on second launch
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app
                .get_webview_window("main")
                .expect("no main window")
                .set_focus();
        }))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_boot_order,
            set_boot_order,
            get_boot_next,
            set_boot_next,
            get_boot_entries,
            discover_entries,
            unset_boot_next,
            get_boot_current,
            set_boot_fw,
            unset_boot_fw,
            get_boot_fw,
            create_shortcut,
            restart_now,
            is_portable,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
