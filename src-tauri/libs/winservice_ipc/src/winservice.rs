use log::error;
use simplelog::*;
use std::env;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::Once;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use windows::Win32::Foundation::{BOOL, ERROR_CALL_NOT_IMPLEMENTED, NO_ERROR, PWSTR};

use windows::Win32::System::Services::{
    CloseServiceHandle, CreateServiceW, OpenSCManagerW, RegisterServiceCtrlHandlerExW,
    SetServiceObjectSecurity, SetServiceStatus, SC_MANAGER_CREATE_SERVICE, SERVICE_ACCEPT_STOP,
    SERVICE_ALL_ACCESS, SERVICE_CONTROL_INTERROGATE, SERVICE_CONTROL_STOP, SERVICE_DEMAND_START,
    SERVICE_ERROR_NORMAL, SERVICE_RUNNING, SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_STOPPED,
    SERVICE_WIN32_OWN_PROCESS,
};
use windows::Win32::System::Services::{StartServiceCtrlDispatcherW, SERVICE_TABLE_ENTRYW};
pub struct ServiceContext {
    pub stop_flag: Arc<AtomicBool>,
}

fn to_wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// Runs a Windows service, calling `service_main` in a new thread.
/// `service_main` receives a `ServiceContext` with a stop flag.
pub fn run_service<F>(service_name: &str, service_main: F) -> windows::core::Result<()>
where
    F: FnOnce(ServiceContext) + Send + 'static,
{
    let stop_service = Arc::new(AtomicBool::new(false));
    let stop_service_clone = Arc::clone(&stop_service);

    unsafe extern "system" fn service_handler(
        control: u32,
        _event_type: u32,
        _event_data: *mut std::ffi::c_void,
        context: *mut std::ffi::c_void,
    ) -> u32 {
        let stop_service = &*(context as *const Arc<AtomicBool>);
        match control {
            SERVICE_CONTROL_STOP | SERVICE_CONTROL_INTERROGATE => {
                stop_service.store(true, Ordering::SeqCst);
                NO_ERROR
            }
            _ => ERROR_CALL_NOT_IMPLEMENTED,
        }
    }

    let service_name_wide = to_wide_string(service_name);

    let service_status_handle: SERVICE_STATUS_HANDLE = unsafe {
        let handle = RegisterServiceCtrlHandlerExW(
            PWSTR(service_name_wide.as_ptr() as *mut _),
            Some(service_handler),
            Arc::into_raw(stop_service_clone) as *mut _,
        );
        if handle.is_invalid() {
            return Err(windows::core::Error::from_win32());
        }
        handle
    };

    let mut service_status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: SERVICE_RUNNING,
        dwControlsAccepted: SERVICE_ACCEPT_STOP,
        dwWin32ExitCode: NO_ERROR,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: 0,
    };

    // Set service status to running
    unsafe {
        if SetServiceStatus(service_status_handle, &service_status) == BOOL(0) {
            return Err(windows::core::Error::from_win32());
        }
    }

    // Run the user-provided service logic
    let ctx = ServiceContext {
        stop_flag: stop_service,
    };
    let handle = std::thread::spawn(move || service_main(ctx));
    handle.join().unwrap();

    // Set service status to stopped
    service_status.dwCurrentState = SERVICE_STOPPED;
    unsafe {
        if SetServiceStatus(service_status_handle, &service_status) == BOOL(0) {
            return Err(windows::core::Error::from_win32());
        }
    }
    Ok(())
}

static INIT: Once = Once::new();

pub fn init_logging() {
    INIT.call_once(|| {
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(r"D:\service.log")
            .unwrap();
        WriteLogger::init(LevelFilter::Info, Config::default(), log_file).unwrap();
        std::panic::set_hook(Box::new(|panic_info| {
            error!("Panic occurred: {:?}", panic_info);
        }));
    });
}

/// Expects a closure that runs your service logic, e.g. `my_service_main`.
pub fn run_windows_service(service_name: &str, service_main: fn(Vec<OsString>)) {
    init_logging();
    let service_name_wide: Vec<u16> = service_name.encode_utf16().chain(Some(0)).collect();

    unsafe extern "system" fn ffi_service_main(_argc: u32, _argv: *mut PWSTR) {
        let args: Vec<String> = env::args().collect();
        // Use the function pointer passed via static mut
        SERVICE_MAIN_PTR.expect("SERVICE_MAIN_PTR not set")(
            args.into_iter().map(OsString::from).collect(),
        );
    }

    // Use a static mut to pass the function pointer to ffi_service_main
    static mut SERVICE_MAIN_PTR: Option<fn(Vec<OsString>)> = None;
    unsafe {
        SERVICE_MAIN_PTR = Some(service_main);
    }

    let service_table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: PWSTR(service_name_wide.as_ptr() as *mut _),
            lpServiceProc: Some(ffi_service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: PWSTR(ptr::null_mut()),
            lpServiceProc: None,
        },
    ];
    unsafe {
        StartServiceCtrlDispatcherW(service_table.as_ptr()).unwrap();
    }
}

/// Installs a Windows service with the given parameters.
/// Returns Ok(()) on success, or an error if installation fails.
pub fn install_service(
    service_name: &str,
    display_name: &str,
    executable_path: &str,
) -> windows::core::Result<()> {
    let scm_handle = unsafe { OpenSCManagerW(None, None, SC_MANAGER_CREATE_SERVICE) };
    if scm_handle.is_invalid() {
        return Err(windows::core::Error::from_win32());
    }

    let service_name_wide = to_wide_string(service_name);
    let display_name_wide = to_wide_string(display_name);
    let executable_path_wide = to_wide_string(executable_path);

    let service_handle = unsafe {
        CreateServiceW(
            scm_handle,
            PWSTR(service_name_wide.as_ptr() as *mut u16),
            PWSTR(display_name_wide.as_ptr() as *mut u16),
            SERVICE_ALL_ACCESS,
            SERVICE_WIN32_OWN_PROCESS,
            SERVICE_DEMAND_START,
            SERVICE_ERROR_NORMAL,
            PWSTR(executable_path_wide.as_ptr() as *mut u16),
            None,
            std::ptr::null_mut(),
            None,
            None,
            None,
        )
    };

    if service_handle.is_invalid() {
        unsafe { CloseServiceHandle(scm_handle) };
        return Err(windows::core::Error::from_win32());
    }

    // --- Grant SERVICE_START to Everyone, preserving existing DACL (SDDL injection, like Python) ---
    unsafe {
        use std::ptr::null_mut;
        use windows::Win32::Security::DACL_SECURITY_INFORMATION;
        use windows::Win32::System::Services::QueryServiceObjectSecurity;

        // Query the current SDDL string
        let mut needed = 0u32;
        let _ = QueryServiceObjectSecurity(
            service_handle,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            0,
            &mut needed,
        );
        let mut buf = vec![0u8; needed as usize];
        let ok = QueryServiceObjectSecurity(
            service_handle,
            DACL_SECURITY_INFORMATION,
            buf.as_mut_ptr() as *mut _,
            needed,
            &mut needed,
        );
        if ok.as_bool() {
            // Convert security descriptor to SDDL string
            use windows::Win32::Security::Authorization::{
                ConvertSecurityDescriptorToStringSecurityDescriptorW, SDDL_REVISION_1,
            };
            let mut sddl_ptr: windows::Win32::Foundation::PWSTR = PWSTR(null_mut());
            let mut sddl_len = 0u32;
            if ConvertSecurityDescriptorToStringSecurityDescriptorW(
                buf.as_ptr() as *const _,
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut sddl_ptr,
                &mut sddl_len,
            )
            .as_bool()
            {
                let sddl = {
                    // Find the length of the null-terminated UTF-16 string
                    let mut len = 0;
                    let mut ptr = sddl_ptr.0;
                    while *ptr != 0 {
                        len += 1;
                        ptr = ptr.add(1);
                    }
                    let slice = std::slice::from_raw_parts(sddl_ptr.0, len);
                    String::from_utf16_lossy(slice)
                };
                // Inject (A;;RPWPCR;;;WD) for Everyone (WD = World)
                let inject = "(A;;RPWPCR;;;WD)";
                let new_sddl = if let Some(idx) = sddl.find(")S:(") {
                    let insert_at = idx + 1;
                    let mut s = sddl.clone();
                    s.insert_str(insert_at, inject);
                    s
                } else {
                    // If no S:(, just append
                    format!("{}{}", sddl, inject)
                };
                use windows::Win32::Security::Authorization::{
                    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
                };

                let mut new_sd: *mut std::ffi::c_void = null_mut();
                let mut new_sd_len = 0u32;
                let new_sddl_w: Vec<u16> = new_sddl.encode_utf16().chain(Some(0)).collect();
                PWSTR(new_sddl_w.as_ptr() as *mut _);
                if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    PWSTR(new_sddl_w.as_ptr() as *mut _),
                    SDDL_REVISION_1,
                    &mut new_sd as *mut _
                        as *mut *mut windows::Win32::Security::SECURITY_DESCRIPTOR,
                    &mut new_sd_len,
                )
                .as_bool()
                {
                    // Set the new security descriptor
                    SetServiceObjectSecurity(
                        service_handle,
                        DACL_SECURITY_INFORMATION,
                        new_sd as *const _,
                    );
                }
            }
        }
    }

    unsafe { CloseServiceHandle(service_handle) };
    unsafe { CloseServiceHandle(scm_handle) };
    Ok(())
}

/// Uninstalls a Windows service using the Windows API.
/// Returns Ok(()) on success, or an error if uninstallation fails.
pub fn uninstall_service(service_name: &str) -> windows::core::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::PWSTR;
    use windows::Win32::System::Services::{
        CloseServiceHandle, DeleteService, OpenSCManagerW, OpenServiceW, SC_MANAGER_CONNECT,
        SERVICE_ALL_ACCESS,
    };

    // Convert service name to wide string
    let service_name_wide: Vec<u16> = OsStr::new(service_name)
        .encode_wide()
        .chain(Some(0))
        .collect();

    unsafe {
        let scm = OpenSCManagerW(None, None, SC_MANAGER_CONNECT);
        if scm.is_invalid() {
            return Err(windows::core::Error::from_win32());
        }
        let service = OpenServiceW(
            scm,
            PWSTR(service_name_wide.as_ptr() as *mut _),
            SERVICE_ALL_ACCESS,
        );
        if service.is_invalid() {
            CloseServiceHandle(scm);
            return Err(windows::core::Error::from_win32());
        }
        let result = DeleteService(service);
        CloseServiceHandle(service);
        CloseServiceHandle(scm);
        if result.as_bool() {
            Ok(())
        } else {
            Err(windows::core::Error::from_win32())
        }
    }
}

#[cfg(windows)]
pub fn start_service(service_name: &str) -> std::io::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    use windows::Win32::Foundation::{ERROR_SERVICE_ALREADY_RUNNING, PWSTR};
    use windows::Win32::System::Services::{
        CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatus, StartServiceW,
        SC_MANAGER_CONNECT, SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START, SERVICE_STATUS,
    };

    // Convert service name to wide string
    let service_name_wide: Vec<u16> = OsStr::new(service_name)
        .encode_wide()
        .chain(Some(0))
        .collect();

    unsafe {
        let scm = OpenSCManagerW(PWSTR(null_mut()), PWSTR(null_mut()), SC_MANAGER_CONNECT);
        if scm.is_invalid() {
            return Err(std::io::Error::last_os_error());
        }
        let service = OpenServiceW(
            scm,
            PWSTR(service_name_wide.as_ptr() as *mut _),
            SERVICE_START | SERVICE_QUERY_STATUS,
        );
        if service.is_invalid() {
            CloseServiceHandle(scm);
            return Err(std::io::Error::last_os_error());
        }
        let mut status = SERVICE_STATUS::default();
        if QueryServiceStatus(service, &mut status).as_bool() {
            if status.dwCurrentState == SERVICE_RUNNING {
                CloseServiceHandle(service);
                CloseServiceHandle(scm);
                return Ok(());
            }
        }
        let result = StartServiceW(service, 0, null_mut());
        let err = std::io::Error::last_os_error();
        CloseServiceHandle(service);
        CloseServiceHandle(scm);
        if result.as_bool() || err.raw_os_error() == Some(ERROR_SERVICE_ALREADY_RUNNING as i32) {
            Ok(())
        } else {
            Err(err)
        }
    }
}
