use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use std::sync::OnceLock;

#[cfg(target_os = "windows")]
static IS_PORTABLE: OnceLock<bool> = OnceLock::new();

#[cfg(target_os = "windows")]
pub fn is_portable_mode() -> bool {
    *IS_PORTABLE.get_or_init(|| std::env::args().any(|arg| arg == "--portable"))
}

use windows::{
    core::{Interface, PCWSTR},
    Win32::{
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{IShellLinkW, ShellLink},
    },
};

fn to_wide<S: AsRef<std::ffi::OsStr>>(s: S) -> Vec<u16> {
    std::ffi::OsStr::new(s.as_ref())
        .encode_wide()
        .chain(Some(0))
        .collect()
}

#[cfg(target_os = "windows")]
pub fn create_shortcut_on_desktop(
    cli_path: &std::path::Path,
    entry_id: u16,
    restart: bool,
    shortcut_name: &str,
) -> Result<(), String> {
    use std::path::PathBuf;
    use windows::Win32::UI::Shell::{FOLDERID_Desktop, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

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
        // Don't forget to free the memory!

        windows::Win32::System::Com::CoTaskMemFree(Some(desktop_ptr.0 as _));

        let mut shortcut_path = PathBuf::from(desktop);
        shortcut_path.push(format!("{shortcut_name}.lnk"));

        let mut args = format!("--set-boot-next {}", entry_id);
        if restart {
            args.push_str(" --reboot");
        }

        // Use CoCreateInstance to create the ShellLink COM object
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
