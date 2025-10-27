use crate::types::{CliCommand, CommandResponse};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock};

static CLI_PROCESS: OnceLock<Mutex<Option<CliProcess>>> = OnceLock::new();

pub struct CliProcessGuard(MutexGuard<'static, Option<CliProcess>>);

impl CliProcessGuard {
    pub fn send_command<T: for<'a> Deserialize<'a>>(
        &mut self,
        cmd: &CliCommand,
    ) -> Result<T, String> {
        self.0
            .as_mut()
            .ok_or("CLI not initialized")?
            .send_command(cmd)
    }

    pub fn send_command_unit(&mut self, cmd: &CliCommand) -> Result<(), String> {
        self.0
            .as_mut()
            .ok_or("CLI not initialized")?
            .send_command_unit(cmd)
    }
}

pub struct CliProcess {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl CliProcess {
    fn start() -> Result<Self, String> {
        let executable_path = std::env::current_exe().map_err(|e| e.to_string())?;

        #[cfg(target_os = "linux")]
        let mut cmd = {
            let mut c = Command::new("pkexec");
            c.arg(&executable_path);
            c.arg("--cli");
            c.arg("--daemon");
            c
        };

        #[cfg(target_os = "windows")]
        let mut cmd = {
            use crate::windows::is_portable_mode;
            let mut c = Command::new(&executable_path);
            c.arg("--cli");
            if is_portable_mode() {
                // In portable mode, create the unelevated pipe server
                c.arg("/pipe_server");
            } else {
                // In service mode, start service and create pipe server
                c.arg("/service_manager");
            }
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                c.creation_flags(CREATE_NO_WINDOW);
            }
            c
        };

        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Failed to start CLI: {e}"))?;

        let stdin = child.stdin.take().ok_or("Failed to open CLI stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to open CLI stdout")?;
        Ok(Self {
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    pub fn send_command<T: for<'a> Deserialize<'a>>(
        &mut self,
        cmd: &CliCommand,
    ) -> Result<T, String> {
        let args_vec = cmd.to_args();
        let args_ref: Vec<&str> = args_vec.iter().map(|s| s.as_str()).collect();
        let cmd_json = serde_json::to_string(&args_ref).map_err(|e| e.to_string())?;
        writeln!(self.stdin, "{cmd_json}").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;

        let mut resp_line = String::new();
        self.stdout
            .read_line(&mut resp_line)
            .map_err(|e| e.to_string())?;
        let resp: CommandResponse = serde_json::from_str(&resp_line).map_err(|e| e.to_string())?;
        if resp.code == 0 {
            serde_json::from_str(&resp.message).map_err(|e| e.to_string())
        } else {
            Err(resp.message)
        }
    }

    pub fn send_command_unit(&mut self, cmd: &CliCommand) -> Result<(), String> {
        let args_vec = cmd.to_args();
        let args_ref: Vec<&str> = args_vec.iter().map(|s| s.as_str()).collect();
        let cmd_json = serde_json::to_string(&args_ref).map_err(|e| e.to_string())?;
        writeln!(self.stdin, "{cmd_json}").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;

        let mut resp_line = String::new();
        self.stdout
            .read_line(&mut resp_line)
            .map_err(|e| e.to_string())?;
        let resp: CommandResponse = serde_json::from_str(&resp_line).map_err(|e| e.to_string())?;
        if resp.code == 0 {
            Ok(())
        } else {
            Err(resp.message)
        }
    }
}

#[cfg(target_os = "windows")]
pub fn get_cli() -> Result<CliProcessGuard, String> {
    use crate::windows::is_portable_mode;

    // Initialize the OnceLock if it hasn't been initialized yet
    let mutex = CLI_PROCESS.get_or_init(|| Mutex::new(None));

    let mut guard = mutex
        .lock()
        .map_err(|_| "Failed to lock CLI process".to_string())?;

    if guard.is_none() {
        // In portable mode, we need to launch an elevated connector before creating the pipe server
        if is_portable_mode() {
            // Launch the elevated connector process first
            let executable_path = std::env::current_exe().expect("Failed to get exe path");

            // Use ShellExecuteW to launch with elevation
            use std::os::windows::ffi::OsStrExt;
            use windows::core::PCWSTR;
            use windows::Win32::UI::Shell::ShellExecuteW;
            use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

            let exe_wide: Vec<u16> = executable_path
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
            let args = "--cli /elevated_connector";
            let args_wide: Vec<u16> = args.encode_utf16().chain(Some(0)).collect();
            let verb = "runas";
            let verb_wide: Vec<u16> = verb.encode_utf16().chain(Some(0)).collect();

            unsafe {
                let result = ShellExecuteW(
                    None,
                    PCWSTR::from_raw(verb_wide.as_ptr()),
                    PCWSTR::from_raw(exe_wide.as_ptr()),
                    PCWSTR::from_raw(args_wide.as_ptr()),
                    None,
                    SW_HIDE,
                );

                // ShellExecuteW returns > 32 on success
                if result.0 as i32 <= 32 {
                    return Err(format!(
                        "Failed to launch elevated connector: error code {}",
                        result.0 as i32
                    ));
                }
            }

            // Give the elevated process a moment to start
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        // Now start the unelevated pipe server (or service manager)
        let cli_process = CliProcess::start()?;
        *guard = Some(cli_process);
    }

    Ok(CliProcessGuard(guard))
}
