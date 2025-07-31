use log::error;
use simplelog::*;
use std::env;
use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::process::Command;
use std::ptr;
use std::sync::Once;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::{BOOL, ERROR_CALL_NOT_IMPLEMENTED, NO_ERROR, PSID, PWSTR};
use windows::Win32::Security::{
    self, AddAccessAllowedAce, AllocateAndInitializeSid, FreeSid, InitializeSecurityDescriptor,
    SetSecurityDescriptorDacl, ACL, ACL_REVISION, DACL_SECURITY_INFORMATION, SECURITY_DESCRIPTOR,
    SID,
};



use windows::Win32::System::SystemServices::{SECURITY_DESCRIPTOR_REVISION, SECURITY_WORLD_RID,
};
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
pub fn _install_service(
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

    // --- Grant SERVICE_START to Everyone ---
    unsafe {
        // Create SID for Everyone
        let mut everyone_sid: PSID = PSID::default();
        let mut world_auth = windows::Win32::Security::SID_IDENTIFIER_AUTHORITY { Value: [0, 0, 0, 0, 0, 1] };
        let result = AllocateAndInitializeSid(
            &world_auth as *const _,
            1,
            SECURITY_WORLD_RID.try_into().unwrap(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut everyone_sid,
        );
        if result.as_bool() {
            // Create a new ACL with SERVICE_START access for Everyone
            let acl_size = std::mem::size_of::<ACL>() as u32
                + std::mem::size_of::<windows::Win32::Security::ACCESS_ALLOWED_ACE>() as u32
                + Security::GetLengthSid(everyone_sid) as u32
                - std::mem::size_of::<u32>() as u32;
            let mut acl = vec![0u8; acl_size as usize];
            let acl_ptr = acl.as_mut_ptr() as *mut ACL;
            Security::InitializeAcl(acl_ptr, acl_size, ACL_REVISION);

            AddAccessAllowedAce(
                acl_ptr,
                ACL_REVISION,
                windows::Win32::System::Services::SERVICE_START,
                everyone_sid,
            );

            // Create and initialize a security descriptor
            let mut sd = SECURITY_DESCRIPTOR::default();
            InitializeSecurityDescriptor(&mut sd, SECURITY_DESCRIPTOR_REVISION);

            SetSecurityDescriptorDacl(&mut sd, BOOL(1), acl_ptr, BOOL(0));

            // Set the security descriptor on the service
            SetServiceObjectSecurity(service_handle, DACL_SECURITY_INFORMATION, &sd);

            FreeSid(everyone_sid);
        }
    }

    unsafe { CloseServiceHandle(service_handle) };
    unsafe { CloseServiceHandle(scm_handle) };
    Ok(())
}


/// Installs a Windows service using `sc.exe` and makes it startable by normal users.
/// Returns Ok(()) on success, or an error if installation fails.
pub fn install_service(
    service_name: &str,
    display_name: &str,
    executable_path: &str,
) -> windows::core::Result<()> {
    // 1. Create the service
    let output = Command::new("sc.exe")
        .args([
            "create",
            service_name,
            &format!("binPath={}", format!("\"{}\"", executable_path)),
            &format!("DisplayName={}", display_name),
            "start=", "demand",
        ])
        .output()
        .map_err(|e| windows::core::Error::new(windows::core::HRESULT(1), format!("Failed to run sc.exe: {e}").into()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(windows::core::Error::new(
            windows::core::HRESULT(1),
            format!("sc.exe failed: {}\n{}", stdout, stderr).into(),
        ));
    }

    // 2. Get the current security descriptor
    let output = Command::new("sc.exe")
        .args(["sdshow", service_name])
        .output()
        .map_err(|e| windows::core::Error::new(windows::core::HRESULT(1), format!("Failed to run sc.exe sdshow: {e}").into()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(windows::core::Error::new(
            windows::core::HRESULT(1),
            format!("sc.exe sdshow failed: {}", stderr).into(),
        ));
    }

    let mut sddl = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // 3. Inject Users group SERVICE_START permission
    // SDDL for: (A;;RPWPCR;;;S-1-5-32-545)
    // Insert before the last ')S:(' if present, else append
    let inject_str = "(A;;RPWPCR;;;S-1-5-32-545)";
    if let Some(idx) = sddl.find(")S:(") {
        sddl.insert_str(idx + 1, inject_str);
    } else {
        sddl.push_str(inject_str);
    }

    // 4. Set the new security descriptor
    let output = Command::new("sc.exe")
        .args(["sdset", service_name, &sddl])
        .output()
        .map_err(|e| windows::core::Error::new(windows::core::HRESULT(1), format!("Failed to run sc.exe sdset: {e}").into()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(windows::core::Error::new(
            windows::core::HRESULT(1),
            format!("sc.exe sdset failed: {}", stderr).into(),
        ));
    }

    Ok(())
}


/// Uninstalls a Windows service using `sc.exe`.
/// Returns Ok(()) on success, or an error if uninstallation fails.
pub fn uninstall_service(service_name: &str) -> windows::core::Result<()> {
    let output = std::process::Command::new("sc.exe")
        .args(["delete", service_name])
        .output()
        .map_err(|e| windows::core::Error::new(
            windows::core::HRESULT(1),
            format!("Failed to run sc.exe delete: {e}").into(),
        ))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(windows::core::Error::new(
            windows::core::HRESULT(1),
            format!("sc.exe delete failed: {}", stderr).into(),
        ));
    }

    Ok(())
}