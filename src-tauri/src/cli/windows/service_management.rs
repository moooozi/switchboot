//! Service install/uninstall/start/stop helpers built on `windows-service`.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};
use windows_service::service::{ServiceAccess, ServiceState};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

pub type Result<T> = std::result::Result<T, ServiceManagementError>;

#[derive(Debug)]
pub enum ServiceManagementError {
    WindowsService(windows_service::Error),
    Timeout(String),
    Io(std::io::Error),
}

impl From<windows_service::Error> for ServiceManagementError {
    fn from(err: windows_service::Error) -> Self {
        ServiceManagementError::WindowsService(err)
    }
}

impl From<std::io::Error> for ServiceManagementError {
    fn from(err: std::io::Error) -> Self {
        ServiceManagementError::Io(err)
    }
}

impl std::fmt::Display for ServiceManagementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceManagementError::WindowsService(e) => write!(f, "Windows service error: {}", e),
            ServiceManagementError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            ServiceManagementError::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for ServiceManagementError {}

pub struct ServiceConfig {
    pub name: OsString,
    pub display_name: OsString,
    pub executable_path: PathBuf,
    pub launch_arguments: Vec<OsString>,
    pub grant_start_to_everyone: bool,
}

/// Install a new service configured for manual (on-demand) start.
pub fn install_service(config: ServiceConfig) -> Result<()> {
    use windows_service::service::{
        ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType,
    };

    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    let service_info = ServiceInfo {
        name: config.name,
        display_name: config.display_name,
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::OnDemand,
        error_control: ServiceErrorControl::Normal,
        executable_path: config.executable_path,
        launch_arguments: config.launch_arguments,
        dependencies: vec![],
        account_name: None, // LocalSystem
        account_password: None,
    };

    // WRITE_DAC is required to modify the security descriptor below.
    let service = manager.create_service(
        &service_info,
        ServiceAccess::CHANGE_CONFIG | ServiceAccess::WRITE_DAC | ServiceAccess::READ_CONTROL,
    )?;

    if config.grant_start_to_everyone {
        eprintln!("[INSTALL] Granting Everyone permission to start the service...");
        match grant_start_permission_to_everyone(&service) {
            Ok(_) => eprintln!("[INSTALL] Successfully granted permissions to Everyone"),
            Err(e) => {
                eprintln!("[INSTALL ERROR] Failed to grant permissions: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}

/// Stop (if running) and remove a service, waiting up to 10s for removal.
pub fn uninstall_service(service_name: &str, stop_if_running: bool) -> Result<()> {
    if stop_if_running {
        // The service may already be stopped or stopping; ignore those errors.
        let _ = stop_service(service_name);
    }

    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(service_name, ServiceAccess::DELETE)?;

    service.delete()?;

    // Release the handle so the SCM can actually delete the service.
    drop(service);

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        let service_exists = manager
            .open_service(service_name, ServiceAccess::QUERY_STATUS)
            .is_ok();

        if !service_exists {
            break;
        }
        sleep(Duration::from_millis(200));
    }

    Ok(())
}

/// Start a service, optionally waiting up to `timeout_secs` for the Running state.
pub fn start_service(service_name: &str, timeout_secs: Option<u64>) -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(
        service_name,
        ServiceAccess::START | ServiceAccess::QUERY_STATUS,
    )?;

    let status = service.query_status()?;
    if status.current_state == ServiceState::Running {
        return Ok(());
    }

    service.start::<&OsStr>(&[])?;

    if let Some(timeout) = timeout_secs {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(timeout) {
            let status = service.query_status()?;
            if status.current_state == ServiceState::Running {
                return Ok(());
            }
            sleep(Duration::from_millis(100));
        }

        let status = service.query_status()?;
        if status.current_state != ServiceState::Running {
            return Err(ServiceManagementError::Timeout(format!(
                "Service did not reach RUNNING state within {} seconds",
                timeout
            )));
        }
    }

    Ok(())
}

/// Stop a running service, waiting up to 10s for it to stop.
pub fn stop_service(service_name: &str) -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(
        service_name,
        ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
    )?;

    let status = service.query_status()?;
    if status.current_state == ServiceState::Stopped {
        return Ok(());
    }

    service.stop()?;

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        let status = service.query_status()?;
        if status.current_state == ServiceState::Stopped {
            return Ok(());
        }
        sleep(Duration::from_millis(200));
    }

    Ok(())
}

/// Resolve a service's executable path, stripping any bundled arguments.
///
/// SCM stores the binary path and its arguments as a single string, so this parses
/// out just the executable, handling quoted paths.
pub fn get_service_binary_path(service_name: &str) -> Option<PathBuf> {
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).ok()?;

    let service = manager
        .open_service(service_name, ServiceAccess::QUERY_CONFIG)
        .ok()?;

    let config = service.query_config().ok()?;

    let path_str = config.executable_path.to_string_lossy();
    let path_str = path_str.trim();

    // Quoted path: take everything up to the closing quote.
    if path_str.starts_with('"') {
        if let Some(end_quote_pos) = path_str[1..].find('"') {
            let exe_path = &path_str[1..end_quote_pos + 1];
            return Some(PathBuf::from(exe_path));
        }
    }

    // Otherwise assume the first whitespace-delimited token is the path.
    let exe_path = path_str.split_whitespace().next()?;
    Some(PathBuf::from(exe_path))
}

/// Grant Everyone (WD) permission to start/stop the service via DACL manipulation.
///
/// By default only administrators can start services. The injected ACE
/// `(A;;RPWPDTLOCRRC;;;WD)` grants Everyone start (RP), stop (WP),
/// pause/continue (DT), interrogate (LO), user-defined control (CR), and
/// read control (RC).
fn grant_start_permission_to_everyone(service: &windows_service::service::Service) -> Result<()> {
    use std::ptr;
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW,
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
    use windows::Win32::System::Services::{
        QueryServiceObjectSecurity, SC_HANDLE, SetServiceObjectSecurity,
    };
    use windows::core::PWSTR;

    unsafe {
        let service_handle = SC_HANDLE(service.raw_handle() as *mut _);

        let mut needed = 0u32;
        let _ = QueryServiceObjectSecurity(
            service_handle,
            DACL_SECURITY_INFORMATION.0,
            None,
            0,
            &mut needed,
        );

        if needed == 0 {
            return Ok(());
        }

        let mut buf = vec![0u8; needed as usize];
        QueryServiceObjectSecurity(
            service_handle,
            DACL_SECURITY_INFORMATION.0,
            Some(PSECURITY_DESCRIPTOR(buf.as_mut_ptr() as *mut _)),
            needed,
            &mut needed,
        )
        .map_err(|e| {
            ServiceManagementError::WindowsService(windows_service::Error::Winapi(
                std::io::Error::from_raw_os_error(e.code().0),
            ))
        })?;

        let mut sddl_ptr: PWSTR = PWSTR(ptr::null_mut());
        let mut sddl_len = 0u32;

        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            PSECURITY_DESCRIPTOR(buf.as_ptr() as *mut _),
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut sddl_ptr,
            Some(&mut sddl_len),
        )
        .map_err(|e| {
            ServiceManagementError::WindowsService(windows_service::Error::Winapi(
                std::io::Error::from_raw_os_error(e.code().0),
            ))
        })?;

        let sddl = {
            let mut len = 0;
            let mut ptr = sddl_ptr.0;
            while *ptr != 0 {
                len += 1;
                ptr = ptr.add(1);
            }
            let slice = std::slice::from_raw_parts(sddl_ptr.0, len);
            String::from_utf16_lossy(slice)
        };

        let inject = "(A;;RPWPDTLOCRRC;;;WD)";
        let new_sddl = if let Some(idx) = sddl.find(")S:(") {
            let insert_at = idx + 1;
            let mut s = sddl.clone();
            s.insert_str(insert_at, inject);
            s
        } else {
            format!("{}{}", sddl, inject)
        };

        let mut new_sd: *mut std::ffi::c_void = ptr::null_mut();
        let mut new_sd_len = 0u32;
        let new_sddl_w: Vec<u16> = new_sddl.encode_utf16().chain(Some(0)).collect();

        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PWSTR(new_sddl_w.as_ptr() as *mut _),
            SDDL_REVISION_1,
            &mut new_sd as *mut _ as *mut PSECURITY_DESCRIPTOR,
            Some(&mut new_sd_len),
        )
        .map_err(|e| {
            ServiceManagementError::WindowsService(windows_service::Error::Winapi(
                std::io::Error::from_raw_os_error(e.code().0),
            ))
        })?;

        SetServiceObjectSecurity(
            service_handle,
            DACL_SECURITY_INFORMATION,
            PSECURITY_DESCRIPTOR(new_sd),
        )
        .map_err(|e| {
            ServiceManagementError::WindowsService(windows_service::Error::Winapi(
                std::io::Error::from_raw_os_error(e.code().0),
            ))
        })?;

        if !new_sd.is_null() {
            let _ = LocalFree(Some(HLOCAL(new_sd)));
        }
        if !sddl_ptr.0.is_null() {
            let _ = LocalFree(Some(HLOCAL(sddl_ptr.0 as *mut _)));
        }
    }

    Ok(())
}
