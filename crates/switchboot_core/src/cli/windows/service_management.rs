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

const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(10);

fn has_code(error: &ServiceManagementError, code: windows::Win32::Foundation::WIN32_ERROR) -> bool {
    matches!(error, ServiceManagementError::WindowsService(windows_service::Error::Winapi(error))
        if error.raw_os_error() == Some(code.0 as i32))
}

fn removal_status(result: Result<()>) -> Result<bool> {
    use windows::Win32::Foundation::{
        ERROR_SERVICE_DOES_NOT_EXIST, ERROR_SERVICE_MARKED_FOR_DELETE,
    };
    match result {
        Ok(()) => Ok(false),
        Err(error) if has_code(&error, ERROR_SERVICE_DOES_NOT_EXIST) => Ok(true),
        Err(error) if has_code(&error, ERROR_SERVICE_MARKED_FOR_DELETE) => Ok(false),
        Err(error) => Err(error),
    }
}

fn wait_until(
    description: &str,
    timeout: Duration,
    mut ready: impl FnMut() -> Result<bool>,
    mut pause: impl FnMut(Duration),
) -> Result<()> {
    let started = Instant::now();
    loop {
        if ready()? {
            return Ok(());
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(ServiceManagementError::Timeout(description.to_owned()));
        }
        pause(remaining.min(Duration::from_millis(200)));
    }
}

trait StopControl {
    fn state(&self) -> Result<ServiceState>;
    fn request_stop(&self) -> Result<()>;
}

impl StopControl for windows_service::service::Service {
    fn state(&self) -> Result<ServiceState> {
        Ok(self.query_status()?.current_state)
    }
    fn request_stop(&self) -> Result<()> {
        self.stop()?;
        Ok(())
    }
}

fn stop_existing(
    service: &impl StopControl,
    timeout: Duration,
    pause: impl FnMut(Duration),
) -> Result<()> {
    use windows::Win32::Foundation::{ERROR_SERVICE_CANNOT_ACCEPT_CTRL, ERROR_SERVICE_NOT_ACTIVE};
    match service.state()? {
        ServiceState::Stopped => return Ok(()),
        ServiceState::StopPending => {}
        _ => {
            if let Err(error) = service.request_stop() {
                // A concurrent stop can race our query. Accept it only after
                // confirming the service is actually stopped or stopping.
                if !(has_code(&error, ERROR_SERVICE_NOT_ACTIVE)
                    || has_code(&error, ERROR_SERVICE_CANNOT_ACCEPT_CTRL))
                    || !matches!(
                        service.state()?,
                        ServiceState::Stopped | ServiceState::StopPending
                    )
                {
                    return Err(error);
                }
            }
        }
    }
    wait_until(
        "Service did not reach STOPPED state within 10 seconds",
        timeout,
        || Ok(service.state()? == ServiceState::Stopped),
        pause,
    )
}

trait Removal {
    fn stop(&self) -> Result<()>;
    fn delete(&self) -> Result<()>;
    fn release(&mut self);
    fn is_removed(&self) -> Result<bool>;
}

struct ManagedRemoval<'a> {
    manager: ServiceManager,
    service: Option<windows_service::service::Service>,
    name: &'a str,
}

impl Removal for ManagedRemoval<'_> {
    fn stop(&self) -> Result<()> {
        if let Some(service) = &self.service {
            stop_existing(service, LIFECYCLE_TIMEOUT, sleep)?;
        }
        Ok(())
    }
    fn delete(&self) -> Result<()> {
        use windows::Win32::Foundation::ERROR_SERVICE_MARKED_FOR_DELETE;
        if let Some(service) = &self.service {
            match service.delete().map_err(ServiceManagementError::from) {
                Ok(()) => {}
                Err(error) if has_code(&error, ERROR_SERVICE_MARKED_FOR_DELETE) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
    fn release(&mut self) {
        // SCM cannot finish deletion while this handle is still open.
        self.service = None;
    }
    fn is_removed(&self) -> Result<bool> {
        removal_status(
            self.manager
                .open_service(self.name, ServiceAccess::QUERY_STATUS)
                .map(|_| ())
                .map_err(ServiceManagementError::from),
        )
    }
}

fn remove_existing(
    removal: &mut impl Removal,
    stop_if_running: bool,
    timeout: Duration,
    pause: impl FnMut(Duration),
) -> Result<()> {
    if stop_if_running {
        removal.stop()?;
    }
    removal.delete()?;
    removal.release();
    wait_until(
        "Service was not removed within 10 seconds",
        timeout,
        || removal.is_removed(),
        pause,
    )
}

/// Stop (if requested) and remove a service. Each phase has a 10-second deadline.
/// Missing services are already uninstalled; pending deletion must finish first.
pub fn uninstall_service(service_name: &str, stop_if_running: bool) -> Result<()> {
    use windows::Win32::Foundation::{
        ERROR_SERVICE_DOES_NOT_EXIST, ERROR_SERVICE_MARKED_FOR_DELETE,
    };
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let mut access = ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS;
    if stop_if_running {
        access |= ServiceAccess::STOP;
    }
    let service = match manager
        .open_service(service_name, access)
        .map_err(ServiceManagementError::from)
    {
        Ok(service) => Some(service),
        Err(error) if has_code(&error, ERROR_SERVICE_DOES_NOT_EXIST) => return Ok(()),
        Err(error) if has_code(&error, ERROR_SERVICE_MARKED_FOR_DELETE) => None,
        Err(error) => return Err(error),
    };
    let mut removal = ManagedRemoval {
        manager,
        service,
        name: service_name,
    };
    remove_existing(&mut removal, stop_if_running, LIFECYCLE_TIMEOUT, sleep)
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

/// Stop a service, confirming STOPPED within 10 seconds. Absence is idempotent.
pub fn stop_service(service_name: &str) -> Result<()> {
    use windows::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST;
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = match manager
        .open_service(
            service_name,
            ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
        )
        .map_err(ServiceManagementError::from)
    {
        Ok(service) => service,
        Err(error) if has_code(&error, ERROR_SERVICE_DOES_NOT_EXIST) => return Ok(()),
        Err(error) => return Err(error),
    };
    stop_existing(&service, LIFECYCLE_TIMEOUT, sleep)
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
    if let Some(unquoted) = path_str.strip_prefix('"')
        && let Some(end_quote_pos) = unquoted.find('"')
    {
        return Some(PathBuf::from(&unquoted[..end_quote_pos]));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;

    fn winapi_error(code: i32) -> ServiceManagementError {
        windows_service::Error::Winapi(std::io::Error::from_raw_os_error(code)).into()
    }

    struct FakeStop {
        states: RefCell<VecDeque<ServiceState>>,
        last: Cell<ServiceState>,
        stop_error: Option<i32>,
        stops: Cell<usize>,
    }
    impl FakeStop {
        fn new(states: &[ServiceState], stop_error: Option<i32>) -> Self {
            Self {
                states: RefCell::new(states.iter().copied().collect()),
                last: Cell::new(ServiceState::Running),
                stop_error,
                stops: Cell::new(0),
            }
        }
    }
    impl StopControl for FakeStop {
        fn state(&self) -> Result<ServiceState> {
            let state = self
                .states
                .borrow_mut()
                .pop_front()
                .unwrap_or(self.last.get());
            self.last.set(state);
            Ok(state)
        }
        fn request_stop(&self) -> Result<()> {
            self.stops.set(self.stops.get() + 1);
            self.stop_error
                .map_or(Ok(()), |code| Err(winapi_error(code)))
        }
    }

    #[test]
    fn stopped_or_stopping_service_does_not_receive_duplicate_stop() {
        for states in [
            vec![ServiceState::Stopped],
            vec![ServiceState::StopPending, ServiceState::Stopped],
        ] {
            let service = FakeStop::new(&states, None);
            stop_existing(&service, Duration::ZERO, |_| {}).unwrap();
            assert_eq!(service.stops.get(), 0);
        }
    }

    #[test]
    fn stop_is_sent_once_and_confirmed_before_success() {
        let service = FakeStop::new(
            &[
                ServiceState::Running,
                ServiceState::StopPending,
                ServiceState::Stopped,
            ],
            None,
        );
        stop_existing(&service, Duration::from_secs(1), |_| {}).unwrap();
        assert_eq!(service.stops.get(), 1);
        assert_eq!(service.last.get(), ServiceState::Stopped);
    }

    #[test]
    fn stop_failure_or_deadline_is_not_reported_as_success() {
        let denied = FakeStop::new(&[ServiceState::Running], Some(5));
        let error = stop_existing(&denied, Duration::ZERO, |_| {}).unwrap_err();
        assert!(has_code(
            &error,
            windows::Win32::Foundation::ERROR_ACCESS_DENIED
        ));
        let pending = FakeStop::new(&[ServiceState::Running, ServiceState::StopPending], None);
        assert!(matches!(
            stop_existing(&pending, Duration::ZERO, |_| {}),
            Err(ServiceManagementError::Timeout(_))
        ));
    }

    #[test]
    fn concurrent_stop_error_requires_confirmed_stopping_state() {
        let stopping = FakeStop::new(
            &[
                ServiceState::Running,
                ServiceState::StopPending,
                ServiceState::Stopped,
            ],
            Some(1061),
        );
        stop_existing(&stopping, Duration::ZERO, |_| {}).unwrap();
        let running = FakeStop::new(&[ServiceState::Running, ServiceState::Running], Some(1061));
        assert!(stop_existing(&running, Duration::ZERO, |_| {}).is_err());
    }

    #[test]
    fn missing_service_is_absent_but_pending_deletion_and_permission_errors_are_not() {
        assert!(removal_status(Err(winapi_error(1060))).unwrap());
        assert!(!removal_status(Err(winapi_error(1072))).unwrap());
        assert!(removal_status(Err(winapi_error(5))).is_err());
        assert!(!removal_status(Ok(())).unwrap());
    }

    struct FakeRemoval {
        actions: RefCell<Vec<&'static str>>,
        stop_error: Option<i32>,
        delete_error: Option<i32>,
        states: RefCell<VecDeque<Result<bool>>>,
        released: bool,
    }
    impl FakeRemoval {
        fn new(states: Vec<Result<bool>>) -> Self {
            Self {
                actions: RefCell::new(Vec::new()),
                stop_error: None,
                delete_error: None,
                states: RefCell::new(states.into()),
                released: false,
            }
        }
    }
    impl Removal for FakeRemoval {
        fn stop(&self) -> Result<()> {
            self.actions.borrow_mut().push("stop");
            self.stop_error
                .map_or(Ok(()), |code| Err(winapi_error(code)))
        }
        fn delete(&self) -> Result<()> {
            self.actions.borrow_mut().push("delete");
            self.delete_error
                .map_or(Ok(()), |code| Err(winapi_error(code)))
        }
        fn release(&mut self) {
            self.actions.borrow_mut().push("release");
            self.released = true;
        }
        fn is_removed(&self) -> Result<bool> {
            assert!(
                self.released,
                "SCM handle must close before checking removal"
            );
            self.actions.borrow_mut().push("query");
            self.states.borrow_mut().pop_front().unwrap_or(Ok(false))
        }
    }

    #[test]
    fn failed_stop_prevents_delete_and_failed_delete_prevents_success() {
        let mut removal = FakeRemoval::new(vec![]);
        removal.stop_error = Some(5);
        assert!(remove_existing(&mut removal, true, Duration::ZERO, |_| {}).is_err());
        assert_eq!(*removal.actions.borrow(), ["stop"]);
        let mut removal = FakeRemoval::new(vec![]);
        removal.delete_error = Some(5);
        assert!(remove_existing(&mut removal, true, Duration::ZERO, |_| {}).is_err());
        assert_eq!(*removal.actions.borrow(), ["stop", "delete"]);
    }

    #[test]
    fn removal_releases_handle_and_waits_for_confirmed_absence() {
        let mut removal = FakeRemoval::new(vec![Ok(false), Ok(true)]);
        remove_existing(&mut removal, true, Duration::from_secs(1), |_| {}).unwrap();
        assert_eq!(
            *removal.actions.borrow(),
            ["stop", "delete", "release", "query", "query"]
        );
    }

    #[test]
    fn persistent_service_and_query_permission_failure_abort_removal() {
        let mut pending = FakeRemoval::new(vec![Ok(false)]);
        assert!(matches!(
            remove_existing(&mut pending, false, Duration::ZERO, |_| {}),
            Err(ServiceManagementError::Timeout(_))
        ));
        let mut denied = FakeRemoval::new(vec![Err(winapi_error(5))]);
        let error = remove_existing(&mut denied, false, Duration::ZERO, |_| {}).unwrap_err();
        assert!(has_code(
            &error,
            windows::Win32::Foundation::ERROR_ACCESS_DENIED
        ));
    }
}
