use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, GetLastError};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub struct Patch {
    token: HANDLE,
    privilege_disable: TOKEN_PRIVILEGES,
}

impl Patch {
    pub fn revert(self) {
        unsafe {
            println!("[privileges] Reverting privileges...");
            AdjustTokenPrivileges(
                self.token,
                false,
                Some(&self.privilege_disable),
                0,
                None,
                None,
            )
            .ok()
            .unwrap();
            CloseHandle(self.token).ok();
        }
    }
}

pub fn patch_current_process_privileges() -> windows::core::Result<Patch> {
    unsafe {
        println!("[privileges] Getting current process handle...");
        let process = GetCurrentProcess();
        let mut token = HANDLE::default();
        println!("[privileges] Opening process token...");
        OpenProcessToken(process, TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token)
            .map_err(|e| {
                println!("[privileges] OpenProcessToken failed: {:?}", e);
                e
            })?;

        let mut luid = Default::default();
        let priv_name: Vec<u16> = "SeSystemEnvironmentPrivilege\0".encode_utf16().collect();
        println!("[privileges] Looking up privilege value...");
        LookupPrivilegeValueW(PCWSTR::null(), PCWSTR(priv_name.as_ptr()), &mut luid)
            .map_err(|e| {
                println!("[privileges] LookupPrivilegeValueW failed: {:?}", e);
                e
            })?;

        let privilege_enable = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        let privilege_disable = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: windows::Win32::Security::TOKEN_PRIVILEGES_ATTRIBUTES(0),
            }],
        };

        println!("[privileges] Adjusting token privileges (enable)...");
        let result = AdjustTokenPrivileges(token, false, Some(&privilege_enable), 0, None, None);
        let last_error = GetLastError();
        if let Err(e) = result {
            println!("[privileges] AdjustTokenPrivileges failed: {:?}", e);
            return Err(e);
        }
        if last_error.0 != 0 {
            println!(
                "[privileges] AdjustTokenPrivileges returned last error: 0x{:08X}",
                last_error.0
            );
            return Err(windows::core::Error::from_win32());
        }
        println!("[privileges] Privilege adjustment succeeded.");
        Ok(Patch {
            token,
            privilege_disable,
        })
    }
}

/// RAII guard for privilege adjustment.
/// Usage:
/// ```rust
/// let _guard = adjust_privileges()?;
/// // ... privileged code ...
/// // privileges are reverted when _guard is dropped
/// ```
pub struct PrivilegeGuard(Option<Patch>);

impl Drop for PrivilegeGuard {
    fn drop(&mut self) {
        if let Some(patch) = self.0.take() {
            patch.revert();
        }
    }
}

pub fn adjust_privileges() -> windows::core::Result<PrivilegeGuard> {
    println!("[privileges] adjust_privileges() called");
    let patch = patch_current_process_privileges()?;
    Ok(PrivilegeGuard(Some(patch)))
}
