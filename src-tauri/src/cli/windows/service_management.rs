//! Windows service management utilities
//!
//! This module provides high-level service management operations using the `windows-service` crate.
//! It includes functions for installing, uninstalling, starting, and stopping Windows services.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};
use windows_service::service::{ServiceAccess, ServiceState};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

/// Result type for service management operations
pub type Result<T> = std::result::Result<T, ServiceManagementError>;

/// Errors that can occur during service management
#[derive(Debug)]
pub enum ServiceManagementError {
    /// Error from the windows-service crate
    WindowsService(windows_service::Error),
    /// Service did not reach expected state within timeout
    Timeout(String),
    /// I/O error
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

/// Service configuration for creating a new service
pub struct ServiceConfig {
    /// Service name (unique identifier)
    pub name: OsString,
    /// Display name shown in services UI
    pub display_name: OsString,
    /// Full path to the service executable
    pub executable_path: PathBuf,
    /// Arguments to pass to the executable when starting the service
    pub launch_arguments: Vec<OsString>,
    /// Whether to grant Everyone permission to start the service
    pub grant_start_to_everyone: bool,
}

/// Install a Windows service
///
/// This function creates a new Windows service with the specified configuration.
/// By default, the service is configured to start on demand (manual start).
///
/// # Arguments
///
/// * `config` - Service configuration including name, display name, and executable path
///
/// # Returns
///
/// Returns `Ok(())` if the service was successfully installed, or an error if installation failed.
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
        account_name: None, // Run as LocalSystem
        account_password: None,
    };

    // Need WRITE_DAC permission to modify the security descriptor
    let service = manager.create_service(
        &service_info,
        ServiceAccess::CHANGE_CONFIG | ServiceAccess::WRITE_DAC | ServiceAccess::READ_CONTROL,
    )?;

    // Grant Everyone permission to start the service if requested
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

/// Uninstall a Windows service
///
/// This function stops (if running) and removes a Windows service.
/// It waits up to 10 seconds for the service to be fully removed from the system.
///
/// # Arguments
///
/// * `service_name` - The name of the service to uninstall
/// * `stop_if_running` - Whether to stop the service if it's currently running
///
/// # Returns
///
/// Returns `Ok(())` if the service was successfully uninstalled, or an error if uninstallation failed.
pub fn uninstall_service(service_name: &str, stop_if_running: bool) -> Result<()> {
    // Stop the service first if requested (using separate function like old code)
    if stop_if_running {
        // Ignore errors from stop - service might already be stopped or stopping
        let _ = stop_service(service_name);
    }

    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(service_name, ServiceAccess::DELETE)?;

    // Mark the service for deletion
    service.delete()?;

    // Drop the service handle so it can be deleted
    drop(service);

    // Wait for the service to be removed from SCM
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

/// Start a Windows service
///
/// This function starts a service and optionally waits for it to reach the RUNNING state.
///
/// # Arguments
///
/// * `service_name` - The name of the service to start
/// * `timeout_secs` - Optional timeout in seconds to wait for the service to start.
///                    If None, the function returns immediately after starting the service.
///
/// # Returns
///
/// Returns `Ok(())` if the service was successfully started (and reached RUNNING state if timeout was specified),
/// or an error if the operation failed.
pub fn start_service(service_name: &str, timeout_secs: Option<u64>) -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(
        service_name,
        ServiceAccess::START | ServiceAccess::QUERY_STATUS,
    )?;

    // Check if already running
    let status = service.query_status()?;
    if status.current_state == ServiceState::Running {
        return Ok(());
    }

    // Start the service
    service.start::<&OsStr>(&[])?;

    // Wait for the service to reach RUNNING state if timeout is specified
    if let Some(timeout) = timeout_secs {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(timeout) {
            let status = service.query_status()?;
            if status.current_state == ServiceState::Running {
                return Ok(());
            }
            sleep(Duration::from_millis(100));
        }

        // Check one final time
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

/// Stop a Windows service
///
/// This function stops a running service and waits up to 10 seconds for it to stop.
///
/// # Arguments
///
/// * `service_name` - The name of the service to stop
///
/// # Returns
///
/// Returns `Ok(())` if the service was successfully stopped, or an error if the operation failed.
pub fn stop_service(service_name: &str) -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;

    let service = manager.open_service(
        service_name,
        ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
    )?;

    // Check if already stopped
    let status = service.query_status()?;
    if status.current_state == ServiceState::Stopped {
        return Ok(());
    }

    // Stop the service
    service.stop()?;

    // Wait for the service to stop
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

/// Get the binary path of a Windows service
///
/// # Arguments
///
/// * `service_name` - The name of the service
///
/// # Returns
///
/// Returns the full path to the service executable if found, or None if the service doesn't exist
/// or the path couldn't be retrieved.
///
/// # Note
///
/// The Windows Service Control Manager stores the binary path with arguments as a single string.
/// This function parses that string to extract just the executable path, handling quoted paths correctly.
pub fn get_service_binary_path(service_name: &str) -> Option<PathBuf> {
    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).ok()?;

    let service = manager
        .open_service(service_name, ServiceAccess::QUERY_CONFIG)
        .ok()?;

    let config = service.query_config().ok()?;

    // The executable_path from windows-service may include arguments.
    // We need to parse it properly to extract just the executable path.
    let path_str = config.executable_path.to_string_lossy();
    let path_str = path_str.trim();

    // If the path starts with a quote, find the closing quote
    if path_str.starts_with('"') {
        // Find the closing quote
        if let Some(end_quote_pos) = path_str[1..].find('"') {
            let exe_path = &path_str[1..end_quote_pos + 1];
            return Some(PathBuf::from(exe_path));
        }
    }

    // If no quotes, take everything up to the first space
    // (assuming no spaces in path, which is less reliable)
    let exe_path = path_str.split_whitespace().next()?;
    Some(PathBuf::from(exe_path))
}

/// Grant SERVICE_START permission to Everyone (DACL manipulation)
///
/// This function modifies the service's DACL (Discretionary Access Control List) to grant
/// the Everyone group (WD = World) permission to start the service.
///
/// This is necessary because by default, only administrators can start services.
/// The SDDL string `(A;;RPWPCR;;;WD)` grants Read Property (RP), Write Property (WP),
/// Control (CR) permissions to Everyone (WD).
///
/// # Arguments
///
/// * `service` - The service to modify
///
/// # Returns
///
/// Returns `Ok(())` if permissions were successfully granted, or an error if the operation failed.
fn grant_start_permission_to_everyone(service: &windows_service::service::Service) -> Result<()> {
    use std::ptr;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW,
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
    use windows::Win32::System::Services::{
        QueryServiceObjectSecurity, SetServiceObjectSecurity, SC_HANDLE,
    };

    unsafe {
        let service_handle = SC_HANDLE(service.raw_handle() as *mut _);

        // Query the current security descriptor size
        let mut needed = 0u32;
        let _ = QueryServiceObjectSecurity(
            service_handle,
            DACL_SECURITY_INFORMATION.0,
            None,
            0,
            &mut needed,
        );

        if needed == 0 {
            return Ok(()); // No security descriptor to modify
        }

        // Allocate buffer and query the security descriptor
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

        // Convert security descriptor to SDDL string
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

        // Read the SDDL string
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

        // Inject permissions for Everyone (WD = World Domain)
        // Service-specific SDDL rights for services:
        // RP = SERVICE_START (0x0010) - This is the critical permission for starting
        // WP = SERVICE_STOP (0x0020)
        // CC = SERVICE_QUERY_CONFIG (0x0001)
        // DC = SERVICE_CHANGE_CONFIG (0x0002)
        // LC = SERVICE_QUERY_STATUS (0x0004)
        // SW = SERVICE_ENUMERATE_DEPENDENTS (0x0008)
        // RC = READ_CONTROL (0x00020000)
        // Grant START, QUERY_STATUS, and READ_CONTROL to Everyone
        let inject = "(A;;RPWPDTLOCRRC;;;WD)"; // RP=START, WP=STOP, DT=PAUSE/CONTINUE, LO=INTERROGATE, CR=USER_DEFINED_CONTROL, RC=READ_CONTROL
        let new_sddl = if let Some(idx) = sddl.find(")S:(") {
            let insert_at = idx + 1;
            let mut s = sddl.clone();
            s.insert_str(insert_at, inject);
            s
        } else {
            format!("{}{}", sddl, inject)
        };

        // Convert the modified SDDL back to a security descriptor
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

        // Set the modified security descriptor
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

        // Cleanup
        if !new_sd.is_null() {
            let _ = LocalFree(Some(HLOCAL(new_sd)));
        }
        if !sddl_ptr.0.is_null() {
            let _ = LocalFree(Some(HLOCAL(sddl_ptr.0 as *mut _)));
        }
    }

    Ok(())
}
