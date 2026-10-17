//! Installer-only OS adapter. No firmware access; service operations remain in NSIS.
use super::{Stack, pop, push, wide};
use std::{
    ffi::c_void,
    fs::OpenOptions,
    os::windows::fs::OpenOptionsExt,
    path::Path,
    ptr::*,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{ProcessStatus::K32EnumProcesses, Threading::*},
    UI::{Shell::CommandLineToArgvW, WindowsAndMessaging::*},
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn error(context: &str) -> String {
    format!("{context}: {}", std::io::Error::last_os_error())
}
fn matching_path(actual: &[u16], expected: &Path) -> bool {
    String::from_utf16_lossy(actual).eq_ignore_ascii_case(&expected.to_string_lossy())
}
unsafe fn process_path(process: HANDLE) -> Option<Vec<u16>> {
    let mut path = vec![0; 32768];
    let mut size = path.len() as u32;
    if QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) == 0 {
        return None;
    }
    path.truncate(size as usize);
    Some(path)
}
unsafe fn shutdown(directory: &str, required_pid: Option<u32>) -> Result<(), String> {
    let expected = Path::new(directory).join("switchboot.exe");
    if !expected.is_file() {
        return Err("The registered Switchboot executable is missing.".into());
    }
    let mut pids = vec![0u32; 32768];
    let mut bytes = 0;
    if let Some(pid) = required_pid {
        pids = vec![pid];
        bytes = 4;
    } else if K32EnumProcesses(pids.as_mut_ptr(), (pids.len() * 4) as u32, &mut bytes) == 0
        || bytes as usize == pids.len() * 4
    {
        return Err(error("Cannot enumerate GUI processes"));
    }
    for pid in pids.into_iter().take(bytes as usize / 4) {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | 0x00100000, 0, pid);
        if process.is_null() {
            if required_pid.is_some() {
                return Err(error("Cannot open upgrade GUI"));
            }
            continue;
        }
        let process = Handle(process);
        let matches = process_path(process.0).is_some_and(|path| matching_path(&path, &expected));
        if !matches {
            if required_pid.is_some() {
                return Err("Upgrade PID does not belong to this installation.".into());
            }
            continue;
        }
        let event = OpenEventW(
            2,
            0,
            wide(&format!("Local\\Switchboot.Upgrade.{pid}")).as_ptr(),
        );
        if event.is_null() {
            if required_pid.is_some() {
                return Err(error("Cannot open GUI shutdown event"));
            }
            continue; // CLI/service instances do not own GUI events.
        }
        let event = Handle(event);
        if SetEvent(event.0) == 0 {
            return Err(error("Cannot request GUI shutdown"));
        }
        if WaitForSingleObject(process.0, 35000) != WAIT_OBJECT_0 {
            return Err("Switchboot did not accept or finish shutdown. Save changes and finish pending operations, then retry. No files or service registration have been changed.".into());
        }
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match OpenOptions::new().write(true).share_mode(0).open(&expected) {
            Ok(file) => {
                drop(file);
                return Ok(());
            }
            Err(e)
                if matches!(e.raw_os_error(), Some(5 | 32 | 33)) && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(e) => {
                return Err(format!(
                    "Switchboot is still in use or cannot be replaced: {e}"
                ));
            }
        }
    }
}

fn validate_directory(directory: &str) -> Result<(), String> {
    let path = Path::new(directory);
    if !path.is_absolute()
        || path.parent().is_none()
        || directory.contains(['\"', '\n', '\r'])
        || directory.encode_utf16().count() > 200
    {
        return Err(
            "Choose an absolute installation subfolder (at most 200 characters), not a drive root."
                .into(),
        );
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Installation paths must not contain '..'.".into());
    }
    let normalized = directory.trim_end_matches(['\\', '/']).to_ascii_lowercase();
    for name in ["WINDIR", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(value) = std::env::var(name) {
            let value = value.trim_end_matches(['\\', '/']).to_ascii_lowercase();
            if normalized == value
                || (name == "WINDIR" && normalized.starts_with(&format!("{value}\\")))
            {
                return Err("Choose a dedicated application folder, not a Windows or Program Files root directory.".into());
            }
        }
    }
    Ok(())
}

/// # Safety
/// Called only through the Unicode NSIS stack ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ValidateDirectory(
    _: HWND,
    size: i32,
    _: *mut u16,
    stack: *mut *mut Stack,
    _: *mut c_void,
) {
    if size <= 0 {
        return;
    }
    let directory = pop(stack, size as usize);
    push(
        stack,
        size as usize,
        &validate_directory(&directory).err().unwrap_or_default(),
    );
}

#[derive(Debug, Default, PartialEq)]
struct Arguments {
    mode: &'static str,
    pid: u32,
    restart: bool,
}
fn parse(args: &[String]) -> Result<Arguments, String> {
    let mut result = Arguments {
        mode: "manual",
        ..Default::default()
    };
    for arg in args.iter().take_while(|a| !a.eq_ignore_ascii_case("/ARGS")) {
        let arg = arg.to_ascii_uppercase();
        match arg.as_str() {
            "/UPDATE" | "/UPGRADE" => {
                let mode = if arg == "/UPDATE" { "legacy" } else { "native" };
                if result.mode != "manual" && result.mode != mode {
                    return Err("/UPDATE and /UPGRADE cannot be combined.".into());
                }
                result.mode = mode;
            }
            "/R" | "/RESTARTAPP" => result.restart = true,
            _ if arg.starts_with("/UPGRADEPID=") => {
                result.pid = arg[12..].parse().map_err(|_| "Invalid upgrade PID.")?
            }
            _ => {}
        }
    }
    if result.mode == "native" && result.pid == 0 {
        return Err("/UPGRADE requires /UPGRADEPID.".into());
    }
    if result.pid != 0 && result.mode != "native" {
        return Err("/UPGRADEPID requires /UPGRADE.".into());
    }
    Ok(result)
}

/// # Safety
/// Called only through the Unicode NSIS stack ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Options(
    _: HWND,
    size: i32,
    _: *mut u16,
    stack: *mut *mut Stack,
    _: *mut c_void,
) {
    if size <= 0 {
        return;
    }
    let raw = pop(stack, size as usize);
    let mut count = 0;
    let argv = CommandLineToArgvW(wide(&format!("setup.exe {raw}")).as_ptr(), &mut count);
    let result = if argv.is_null() {
        Err(error("Cannot parse installer arguments"))
    } else {
        let args = std::slice::from_raw_parts(argv, count as usize)
            .iter()
            .skip(1)
            .map(|p| {
                let mut len = 0;
                while *p.add(len) != 0 {
                    len += 1;
                }
                String::from_utf16_lossy(std::slice::from_raw_parts(*p, len))
            })
            .collect::<Vec<_>>();
        let result = parse(&args);
        LocalFree(argv.cast());
        result
    };
    let (status, args) = match result {
        Ok(args) => (String::new(), args),
        Err(e) => (e, Arguments::default()),
    };
    push(stack, size as usize, if args.restart { "1" } else { "0" });
    push(stack, size as usize, &args.pid.to_string());
    push(stack, size as usize, args.mode);
    push(stack, size as usize, &status);
}

/// # Safety
/// Called only through the Unicode NSIS stack ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn PrepareShutdown(
    _: HWND,
    size: i32,
    _: *mut u16,
    stack: *mut *mut Stack,
    _: *mut c_void,
) {
    if size <= 0 {
        return;
    }
    let directory = pop(stack, size as usize);
    let pid = pop(stack, size as usize);
    let result = match pid.parse::<u32>() {
        Ok(pid) => shutdown(&directory, (pid != 0).then_some(pid)),
        Err(_) => Err("Invalid shutdown PID.".into()),
    };
    push(stack, size as usize, &result.err().unwrap_or_default());
}

/// Launch through the interactive shell's primary token, never Setup's elevated token.
unsafe fn launch(directory: &str) -> Result<(), String> {
    use windows_sys::Win32::Security::*;
    let shell = GetShellWindow();
    let mut pid = 0;
    if shell.is_null() || GetWindowThreadProcessId(shell, &mut pid) == 0 {
        return Err(error("Cannot find original-user shell"));
    }
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if process.is_null() {
        return Err(error("Cannot open original-user shell"));
    }
    let process = Handle(process);
    let mut token = null_mut();
    if OpenProcessToken(process.0, TOKEN_DUPLICATE | TOKEN_QUERY, &mut token) == 0 {
        return Err(error("Cannot open shell token"));
    }
    let token = Handle(token);
    let mut elevation = TOKEN_ELEVATION::default();
    let mut bytes = 0;
    if GetTokenInformation(
        token.0,
        TokenElevation,
        (&mut elevation as *mut TOKEN_ELEVATION).cast(),
        std::mem::size_of::<TOKEN_ELEVATION>() as u32,
        &mut bytes,
    ) == 0
    {
        return Err(error("Cannot verify original-user token"));
    }
    if elevation.TokenIsElevated != 0 {
        return Err("The desktop shell is elevated. Refusing to launch the GUI elevated.".into());
    }
    let mut primary = null_mut();
    if DuplicateTokenEx(
        token.0,
        TOKEN_ALL_ACCESS,
        null(),
        SecurityImpersonation,
        TokenPrimary,
        &mut primary,
    ) == 0
    {
        return Err(error("Cannot duplicate shell token"));
    }
    let primary = Handle(primary);
    use windows_sys::Win32::System::Environment::{
        CreateEnvironmentBlock, DestroyEnvironmentBlock,
    };
    struct Environment(*mut c_void);
    impl Drop for Environment {
        fn drop(&mut self) {
            unsafe {
                DestroyEnvironmentBlock(self.0);
            }
        }
    }
    let mut environment = null_mut();
    if CreateEnvironmentBlock(&mut environment, primary.0, 0) == 0 {
        return Err(error("Cannot create original-user environment"));
    }
    let environment = Environment(environment);
    let executable = Path::new(directory).join("switchboot.exe");
    let mut command = wide(&format!("\"{}\"", executable.display()));
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..std::mem::zeroed()
    };
    let mut info = std::mem::zeroed::<PROCESS_INFORMATION>();
    if CreateProcessWithTokenW(
        primary.0,
        0,
        wide(&executable.to_string_lossy()).as_ptr(),
        command.as_mut_ptr(),
        CREATE_UNICODE_ENVIRONMENT,
        environment.0,
        wide(directory).as_ptr(),
        &startup,
        &mut info,
    ) == 0
    {
        return Err(error("Cannot launch Switchboot as the desktop user"));
    }
    let _process = Handle(info.hProcess);
    let _thread = Handle(info.hThread);
    Ok(())
}

/// # Safety
/// Called only through the Unicode NSIS stack ABI.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Launch(
    _: HWND,
    size: i32,
    _: *mut u16,
    stack: *mut *mut Stack,
    _: *mut c_void,
) {
    if size <= 0 {
        return;
    }
    let directory = pop(stack, size as usize);
    push(
        stack,
        size as usize,
        &launch(&directory).err().unwrap_or_default(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn legacy_boundary_does_not_replay_app_arguments() {
        assert_eq!(
            parse(&args(&[
                "/UPDATE",
                "/P",
                "/R",
                "/ARGS",
                "/UPGRADE",
                "/UPGRADEPID=9"
            ]))
            .unwrap(),
            Arguments {
                mode: "legacy",
                pid: 0,
                restart: true
            }
        );
        assert_eq!(
            parse(&args(&["/ARGS", "/UPDATE", "/R"])).unwrap().mode,
            "manual"
        );
    }
    #[test]
    fn argument_export_preserves_stack_order_and_args_boundary() {
        unsafe {
            let mut stack = null_mut();
            push(
                &mut stack,
                1024,
                r#"/UPDATE /P /R /ARGS \"中文 /UPGRADE\" /UPGRADEPID=9"#,
            );
            Options(null_mut(), 1024, null_mut(), &mut stack, null_mut());
            assert_eq!(pop(&mut stack, 1024), "");
            assert_eq!(pop(&mut stack, 1024), "legacy");
            assert_eq!(pop(&mut stack, 1024), "0");
            assert_eq!(pop(&mut stack, 1024), "1");
            assert!(stack.is_null());
        }
    }
    #[test]
    fn native_requires_an_unambiguous_pid() {
        for raw in [
            vec!["/UPGRADE"],
            vec!["/UPGRADEPID=9"],
            vec!["/UPDATE", "/UPGRADE"],
            vec!["/UPGRADE", "/UPGRADEPID=no"],
        ] {
            assert!(parse(&args(&raw)).is_err());
        }
        assert_eq!(
            parse(&args(&["/UPGRADE", "/UPGRADEPID=42"])).unwrap().pid,
            42
        );
    }
    #[test]
    fn directory_must_be_a_dedicated_absolute_subfolder() {
        for directory in [r"C:\", "relative", r"C:\Apps\..\Windows", "C:\\bad\"path"] {
            assert!(validate_directory(directory).is_err(), "{directory}");
        }
        assert!(validate_directory(r"C:\Apps\Switchboot").is_ok());
    }
    #[test]
    fn path_matching_is_exact_not_prefix() {
        let path = Path::new(r"C:\Program Files\Switchboot\switchboot.exe");
        assert!(matching_path(
            &wide(r"c:\program files\switchboot\SWITCHBOOT.exe")[..path.as_os_str().len()],
            path
        ));
        assert!(!matching_path(&wide(r"C:\Other\switchboot.exe"), path));
    }
}
