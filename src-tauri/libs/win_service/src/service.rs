use log::error;
use simplelog::*;
use std::env;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::{Arc, Once};
use windows::Win32::Foundation::{BOOL, ERROR_CALL_NOT_IMPLEMENTED, NO_ERROR, PWSTR};

use windows::Win32::System::Services::{
    CloseServiceHandle, CreateServiceW, OpenSCManagerW, RegisterServiceCtrlHandlerExW,
    SetServiceObjectSecurity, SetServiceStatus, SC_MANAGER_CREATE_SERVICE, SERVICE_ACCEPT_STOP,
    SERVICE_ALL_ACCESS, SERVICE_CONTROL_INTERROGATE, SERVICE_CONTROL_STOP, SERVICE_DEMAND_START,
    SERVICE_ERROR_NORMAL, SERVICE_RUNNING, SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_STOPPED,
    SERVICE_STOP_PENDING, SERVICE_WIN32_OWN_PROCESS,
};
use windows::Win32::System::Services::{StartServiceCtrlDispatcherW, SERVICE_TABLE_ENTRYW};
pub struct ServiceContext {
    pub ready_notify: Option<Arc<tokio::sync::Notify>>,
    pub stop_notify: Option<Arc<tokio::sync::Notify>>,
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
    run_service_with_readiness(service_name, service_main, false)
}

/// Runs a Windows service with readiness checking.
/// If `wait_for_ready` is true, the service will set its status to START_PENDING
/// until the service_main signals readiness via ready_signal.
pub fn run_service_with_readiness<F>(
    service_name: &str,
    service_main: F,
    wait_for_ready: bool,
) -> windows::core::Result<()>
where
    F: FnOnce(ServiceContext) + Send + 'static,
{
    let stop_notify = Arc::new(tokio::sync::Notify::new());
    let ready_notify = if wait_for_ready {
        Some(Arc::new(tokio::sync::Notify::new()))
    } else {
        None
    };

    // Add a struct to hold both the stop flag and the status handle
    struct HandlerContext {
        status_handle: SERVICE_STATUS_HANDLE,
        stop_notify: Option<Arc<tokio::sync::Notify>>,
    }

    unsafe extern "system" fn service_handler(
        control: u32,
        _event_type: u32,
        _event_data: *mut std::ffi::c_void,
        context: *mut std::ffi::c_void,
    ) -> u32 {
        let ctx = &*(context as *const HandlerContext);
        match control {
            SERVICE_CONTROL_STOP => {
                // Set status to STOP_PENDING
                let status = SERVICE_STATUS {
                    dwServiceType: SERVICE_WIN32_OWN_PROCESS,
                    dwCurrentState: SERVICE_STOP_PENDING,
                    dwControlsAccepted: 0,
                    dwWin32ExitCode: NO_ERROR,
                    dwServiceSpecificExitCode: 0,
                    dwCheckPoint: 0,
                    dwWaitHint: 10000, // 10 seconds
                };
                unsafe {
                    SetServiceStatus(ctx.status_handle, &status);
                }
                if let Some(notify) = &ctx.stop_notify {
                    notify.notify_waiters();
                }

                // Watcher thread: terminate if not stopped after 7 seconds (3 seconds before wait hint)
                let status_handle = ctx.status_handle;
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(7));
                    // If still not stopped, set status to STOPPED and terminate
                    let stopped_status = SERVICE_STATUS {
                        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
                        dwCurrentState: SERVICE_STOPPED,
                        dwControlsAccepted: 0,
                        dwWin32ExitCode: NO_ERROR,
                        dwServiceSpecificExitCode: 0,
                        dwCheckPoint: 0,
                        dwWaitHint: 0,
                    };
                    unsafe {
                        SetServiceStatus(status_handle, &stopped_status);
                    }
                    println!("Watcher: Service did not stop in time. Terminating process.");
                    std::process::exit(1);
                });

                NO_ERROR
            }
            SERVICE_CONTROL_INTERROGATE => NO_ERROR,
            _ => ERROR_CALL_NOT_IMPLEMENTED,
        }
    }

    let service_name_wide = to_wide_string(service_name);

    // Use Box instead of Arc for handler context
    let handler_ctx = Box::new(HandlerContext {
        status_handle: SERVICE_STATUS_HANDLE::default(),
        stop_notify: None,
    });

    unsafe {
        // Register handler, passing pointer to Box
        let ctx_ptr = Box::into_raw(handler_ctx);
        let handle = RegisterServiceCtrlHandlerExW(
            PWSTR(service_name_wide.as_ptr() as *mut _),
            Some(service_handler),
            ctx_ptr as *mut _,
        );
        if handle.is_invalid() {
            // Clean up if registration failed
            let _ = Box::from_raw(ctx_ptr);
            return Err(windows::core::Error::from_win32());
        }
        // Update the status_handle in the original Box
        (*ctx_ptr).status_handle = handle;

        let mut service_status = SERVICE_STATUS {
            dwServiceType: SERVICE_WIN32_OWN_PROCESS,
            dwCurrentState: if wait_for_ready {
                use windows::Win32::System::Services::SERVICE_START_PENDING;
                SERVICE_START_PENDING
            } else {
                SERVICE_RUNNING
            },
            dwControlsAccepted: SERVICE_ACCEPT_STOP,
            dwWin32ExitCode: NO_ERROR,
            dwServiceSpecificExitCode: 0,
            dwCheckPoint: 0,
            dwWaitHint: if wait_for_ready { 30000 } else { 0 }, // 30 seconds wait hint for start pending
        };

        // Set initial service status
        if SetServiceStatus(handle, &service_status) == BOOL(0) {
            return Err(windows::core::Error::from_win32());
        }

        // Run the user-provided service logic
        let ctx = ServiceContext {
            ready_notify: ready_notify.clone(),
            stop_notify: Some(stop_notify.clone()),
        };

        let ready_check = ready_notify.clone();
        let handle_copy = handle;
        // Store the stop_notify in the handler context so the service handler can notify
        (*ctx_ptr).stop_notify = Some(stop_notify.clone());
        let handle_thread = std::thread::spawn(move || {
            service_main(ctx);

            // If we were waiting for readiness, we cannot check an atomic flag anymore.
            // We simply log that the service_main returned without explicit readiness notification.
            if ready_check.is_some() {
                eprintln!("Info: Service main returned; readiness may have been signaled earlier or not at all");
            }
        });

        // If waiting for readiness, monitor the ready signal
        if wait_for_ready {
            if let Some(ready) = ready_notify {
                // Block on readiness or stop notification using a small tokio runtime
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build()
                    .expect("failed to build runtime for readiness wait");
                rt.block_on(async {
                    tokio::select! {
                        _ = ready.notified() => {
                            // Received readiness
                        }
                        _ = stop_notify.notified() => {
                            // Stop requested before readiness
                        }
                    }
                });

                // Update service status to RUNNING when ready (if stop wasn't triggered)
                // We have no direct atomic stop flag now; assume if stop_notify fired, we should not proceed.
                // For simplicity, set RUNNING unconditionally if readiness was signaled.
                service_status.dwCurrentState = SERVICE_RUNNING;
                service_status.dwWaitHint = 0;
                if SetServiceStatus(handle_copy, &service_status) == BOOL(0) {
                    return Err(windows::core::Error::from_win32());
                }
                println!("[SERVICE] Status set to RUNNING after readiness signal");
            }
        }

        handle_thread.join().unwrap();

        // Set service status to stopped
        service_status.dwCurrentState = SERVICE_STOPPED;
        service_status.dwWaitHint = 0;
        if SetServiceStatus(handle, &service_status) == BOOL(0) {
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

/// Stops a Windows service by name. Waits up to 10 seconds for it to stop.
pub fn stop_service(service_name: &str) -> windows::core::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::thread::sleep;
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::PWSTR;
    use windows::Win32::System::Services::{
        CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceStatus,
        SC_MANAGER_CONNECT, SERVICE_ALL_ACCESS, SERVICE_CONTROL_STOP, SERVICE_STATUS,
        SERVICE_STOPPED,
    };

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

        let mut status = SERVICE_STATUS::default();
        if QueryServiceStatus(service, &mut status).as_bool() {
            if status.dwCurrentState != SERVICE_STOPPED {
                ControlService(service, SERVICE_CONTROL_STOP, &mut status);
                // Wait for the service to stop (max 10 seconds)
                let start = Instant::now();
                while status.dwCurrentState != SERVICE_STOPPED
                    && start.elapsed() < Duration::from_secs(10)
                {
                    sleep(Duration::from_millis(200));
                    if !QueryServiceStatus(service, &mut status).as_bool() {
                        break;
                    }
                }
            }
        }

        CloseServiceHandle(service);
        CloseServiceHandle(scm);
        Ok(())
    }
}

/// Uninstalls a Windows service using the Windows API.
/// If `dont_stop_service` is false, the service will be stopped before deletion.
/// Returns Ok(()) on success, or an error if uninstallation fails.
pub fn uninstall_service(
    service_name: &str,
    should_stop_service: bool,
) -> windows::core::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::thread::sleep;
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::{ERROR_SERVICE_MARKED_FOR_DELETE, PWSTR};
    use windows::Win32::System::Services::{
        CloseServiceHandle, DeleteService, OpenSCManagerW, OpenServiceW, SC_MANAGER_CONNECT,
        SERVICE_ALL_ACCESS,
    };

    // Convert service name to wide string
    let service_name_wide: Vec<u16> = OsStr::new(service_name)
        .encode_wide()
        .chain(Some(0))
        .collect();

    // Stop the service first if should_stop_service is true
    if should_stop_service {
        stop_service(service_name)?;
    }

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

        if !result.as_bool() {
            let err = windows::core::Error::from_win32();
            // If already marked for delete, treat as success
            if let Some(code) = err.code().0.checked_abs() {
                if code == ERROR_SERVICE_MARKED_FOR_DELETE as i32 {
                    return Ok(());
                }
            }
            return Err(err);
        }

        // Optionally: Wait for the service to be removed from SCM (max 10 seconds)
        let start = Instant::now();
        loop {
            let scm = OpenSCManagerW(None, None, SC_MANAGER_CONNECT);
            if scm.is_invalid() {
                break;
            }
            let svc = OpenServiceW(
                scm,
                PWSTR(service_name_wide.as_ptr() as *mut _),
                SERVICE_ALL_ACCESS,
            );
            CloseServiceHandle(scm);
            if svc.is_invalid() {
                // Service no longer exists
                break;
            }
            CloseServiceHandle(svc);
            if start.elapsed() > Duration::from_secs(10) {
                break;
            }
            sleep(Duration::from_millis(200));
        }

        Ok(())
    }
}

/// Starts a Windows service by name.
/// If `service_run_timeout` is `Some(timeout_secs)`, this will poll the service status and wait up to `timeout_secs` seconds
/// for the service to reach the RUNNING state before returning. If the timeout is reached, returns a TimedOut error.
/// If `service_run_timeout` is `None`, this will return immediately after starting the service (or if already running).
/// Returns Ok(()) on success, or an error if starting or waiting fails.
#[cfg(windows)]
pub fn start_service(service_name: &str, service_run_timeout: Option<u64>) -> std::io::Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    use std::thread::sleep;
    use std::time::{Duration, Instant};
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

        // Optionally wait for RUNNING state
        let final_result = if result.as_bool()
            || err.raw_os_error() == Some(ERROR_SERVICE_ALREADY_RUNNING as i32)
        {
            if let Some(timeout_secs) = service_run_timeout {
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(timeout_secs) {
                    if QueryServiceStatus(service, &mut status).as_bool() {
                        if status.dwCurrentState == SERVICE_RUNNING {
                            break;
                        }
                    }
                    sleep(Duration::from_millis(10));
                }
                if status.dwCurrentState != SERVICE_RUNNING {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!(
                            "Service did not reach RUNNING state within {} seconds",
                            timeout_secs
                        ),
                    ))
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            }
        } else {
            Err(err)
        };

        CloseServiceHandle(service);
        CloseServiceHandle(scm);
        final_result
    }
}
