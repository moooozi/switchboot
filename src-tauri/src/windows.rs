use crate::types::ShortcutAction;
use std::os::windows::ffi::OsStrExt;

#[cfg(target_os = "windows")]
pub fn is_portable_mode() -> bool {
    use crate::cli::windows::service_management::get_service_binary_path;
    use crate::constants::SERVICE_NAME;

    let current_exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => return true, // default to portable if undeterminable
    };

    let service_binary_path = match get_service_binary_path(SERVICE_NAME) {
        Some(path) => std::path::PathBuf::from(path),
        None => return true, // no installed service => portable mode
    };

    // Portable mode iff the running exe is not beside the installed service binary.
    let service_base = service_binary_path
        .parent()
        .map(|p| p.to_string_lossy().to_lowercase());
    let current_base = current_exe
        .parent()
        .map(|p| p.to_string_lossy().to_lowercase());

    service_base != current_base
}

use windows::{
    Win32::{
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, IPersistFile,
        },
        UI::Shell::{IShellLinkW, ShellLink},
    },
    core::{Interface, PCWSTR},
};

fn to_wide<S: AsRef<std::ffi::OsStr>>(s: S) -> Vec<u16> {
    std::ffi::OsStr::new(s.as_ref())
        .encode_wide()
        .chain(Some(0))
        .collect()
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\' | '/' => '_',
            c if c.is_control() => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(target_os = "windows")]
pub fn create_shortcut_on_desktop(
    cli_path: &std::path::Path,
    action: &ShortcutAction,
    entry_id: Option<u16>,
    restart: bool,
    shortcut_name: &str,
    icon_id: Option<String>,
) -> Result<(), String> {
    use std::path::PathBuf;
    use windows::Win32::UI::Shell::{FOLDERID_Desktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

    unsafe {
        let hr = CoInitializeEx(Some(std::ptr::null_mut()), COINIT_APARTMENTTHREADED);
        if hr.is_err() {
            return Err(format!("CoInitializeEx failed: {hr:?}"));
        }

        let desktop_ptr = SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None)
            .map_err(|_| "Could not get Desktop directory".to_string())?;
        let desktop = {
            use std::os::windows::ffi::OsStringExt;
            let mut len = 0;

            while *desktop_ptr.0.offset(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(desktop_ptr.0, len as usize);
            let os_string = std::ffi::OsString::from_wide(slice);
            PathBuf::from(os_string)
        };

        windows::Win32::System::Com::CoTaskMemFree(Some(desktop_ptr.0 as _));

        let mut shortcut_path = PathBuf::from(desktop);
        shortcut_path.push(format!("{}.lnk", sanitize_filename(shortcut_name)));

        let mut args = match action {
            ShortcutAction::SetBootNext => {
                if let Some(id) = entry_id {
                    format!("--exec set-boot-next {}", id)
                } else {
                    return Err("entry_id required for SetBootNext action".to_string());
                }
            }
            ShortcutAction::SetFirmwareSetup => "--exec set-boot-fw".to_string(),
        };
        if restart {
            args.push_str(" reboot");
        }

        let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("CoCreateInstance failed: {e}"))?;

        shell_link
            .SetPath(PCWSTR::from_raw(
                to_wide(cli_path.display().to_string()).as_ptr(),
            ))
            .map_err(|e| format!("SetPath failed: {e}"))?;
        shell_link
            .SetArguments(PCWSTR::from_raw(to_wide(args).as_ptr()))
            .map_err(|e| format!("SetArguments failed: {e}"))?;
        shell_link
            .SetDescription(PCWSTR::from_raw(
                to_wide("SwitchBoot: Set boot entry and restart").as_ptr(),
            ))
            .map_err(|e| format!("SetDescription failed: {e}"))?;

        // Installed layout: <exe_parent>/resources/icons/ico/<icon>.ico
        let icon_path = if let Some(id) = icon_id {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_parent) = exe_path.parent() {
                    let p = exe_parent
                        .join("resources")
                        .join("icons")
                        .join("ico")
                        .join(format!("{}.ico", id));
                    if p.exists() {
                        p
                    } else {
                        std::path::PathBuf::new()
                    }
                } else {
                    std::path::PathBuf::new()
                }
            } else {
                std::path::PathBuf::new()
            }
        } else {
            std::path::PathBuf::new()
        };

        // Prefer the requested icon, otherwise fall back to the bundled generic icon.
        let final_icon = if icon_path.exists() {
            icon_path
        } else if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_parent) = exe_path.parent() {
                let fb = exe_parent
                    .join("resources")
                    .join("icons")
                    .join("ico")
                    .join("generic.ico");
                if fb.exists() {
                    fb
                } else {
                    std::path::PathBuf::new()
                }
            } else {
                std::path::PathBuf::new()
            }
        } else {
            std::path::PathBuf::new()
        };

        if final_icon.exists() {
            // Keep the wide buffer alive across the FFI call.
            let icon_wide = to_wide(final_icon.display().to_string());
            shell_link
                .SetIconLocation(PCWSTR::from_raw(icon_wide.as_ptr()), 0)
                .map_err(|e| format!("SetIconLocation failed: {e}"))?;
        }

        let persist_file: IPersistFile = shell_link
            .cast()
            .map_err(|e| format!("IPersistFile cast failed: {e}"))?;
        persist_file
            .Save(PCWSTR::from_raw(to_wide(shortcut_path).as_ptr()), true)
            .map_err(|e| format!("Failed to save shortcut: {e}"))?;

        CoUninitialize();
    }

    Ok(())
}
