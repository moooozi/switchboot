use crate::types::{CliCommand, CommandResponse};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock, mpsc};
use std::time::Duration;

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

type Responses = mpsc::Receiver<Result<CommandResponse, String>>;

fn start_io_worker(
    mut stdin: impl Write + Send + 'static,
    stdout: impl Read + Send + 'static,
) -> (mpsc::SyncSender<String>, Responses) {
    let (requests_tx, requests_rx) = mpsc::sync_channel::<String>(1);
    let (responses_tx, responses_rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        for request in requests_rx {
            let response = (|| {
                writeln!(stdin, "{request}").map_err(|e| e.to_string())?;
                stdin.flush().map_err(|e| e.to_string())?;
                let mut line = String::new();
                if stdout.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                    return Err("CLI closed stdout without a response".to_string());
                }
                serde_json::from_str(&line).map_err(|e| format!("Invalid CLI response: {e}"))
            })();
            let failed = response.is_err();
            if responses_tx.send(response).is_err() || failed {
                break;
            }
        }
    });
    (requests_tx, responses_rx)
}

fn exchange(
    requests: &mpsc::SyncSender<String>,
    responses: &Responses,
    request: String,
    timeout: Duration,
) -> Result<CommandResponse, String> {
    requests
        .try_send(request)
        .map_err(|e| format!("CLI request channel unavailable: {e}"))?;
    responses
        .recv_timeout(timeout)
        .map_err(|e| format!("CLI response unavailable: {e}"))?
}

static CLI_PROCESS: OnceLock<Mutex<Option<CliProcess>>> = OnceLock::new();

pub struct CliProcessGuard(MutexGuard<'static, Option<CliProcess>>);

impl CliProcessGuard {
    pub fn send_command<T: for<'a> Deserialize<'a>>(
        &mut self,
        cmd: &CliCommand,
    ) -> Result<T, String> {
        let result = self
            .0
            .as_mut()
            .ok_or("CLI not initialized")?
            .send_command(cmd);
        self.discard_broken_connection();
        result
    }

    pub fn send_command_unit(&mut self, cmd: &CliCommand) -> Result<(), String> {
        let result = self
            .0
            .as_mut()
            .ok_or("CLI not initialized")?
            .send_command_unit(cmd);
        self.discard_broken_connection();
        result
    }

    fn discard_broken_connection(&mut self) {
        if self.0.as_ref().is_some_and(|process| !process.healthy) {
            *self.0 = None;
        }
    }
}

pub struct CliProcess {
    child: Child,
    requests: mpsc::SyncSender<String>,
    responses: Responses,
    healthy: bool,
}

impl Drop for CliProcess {
    fn drop(&mut self) {
        // Closing a stale session must never resend its last firmware command.
        if self.child.kill().is_ok() {
            let _ = self.child.wait();
        } else {
            let _ = self.child.try_wait();
        }
    }
}

impl CliProcess {
    fn request(&mut self, cmd: &CliCommand) -> Result<CommandResponse, String> {
        let json = serde_json::to_string(&cmd.to_args()).map_err(|e| e.to_string())?;
        exchange(&self.requests, &self.responses, json, RESPONSE_TIMEOUT).map_err(|error| {
            self.healthy = false;
            format!("CLI transport failed: {error}. The command may have executed; refresh firmware state before retrying.")
        })
    }

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
                c.arg("/pipe_server");
            } else {
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
        let (requests, responses) = start_io_worker(stdin, stdout);
        Ok(Self {
            child,
            requests,
            responses,
            healthy: true,
        })
    }

    pub fn send_command<T: for<'a> Deserialize<'a>>(
        &mut self,
        cmd: &CliCommand,
    ) -> Result<T, String> {
        let resp = self.request(cmd)?;
        if resp.code == 0 {
            serde_json::from_str(&resp.message).map_err(|e| {
                self.healthy = false;
                format!("Invalid CLI payload: {e}")
            })
        } else {
            Err(resp.message)
        }
    }

    pub fn send_command_unit(&mut self, cmd: &CliCommand) -> Result<(), String> {
        let resp = self.request(cmd)?;
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

    let mutex = CLI_PROCESS.get_or_init(|| Mutex::new(None));

    let mut guard = mutex
        .lock()
        .map_err(|_| "Failed to lock CLI process".to_string())?;

    if let Some(process) = guard.as_mut() {
        match process.child.try_wait() {
            Ok(None) if process.healthy => {}
            _ => *guard = None,
        }
    }

    if guard.is_none() {
        // In portable mode the elevated connector must be launched before the pipe server.
        if is_portable_mode() {
            let executable_path = std::env::current_exe().map_err(|e| e.to_string())?;

            use std::os::windows::ffi::OsStrExt;
            use windows::Win32::UI::Shell::ShellExecuteW;
            use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
            use windows::core::PCWSTR;

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

                // ShellExecuteW returns > 32 on success.
                if result.0 as i32 <= 32 {
                    return Err(format!(
                        "Failed to launch elevated connector: error code {}",
                        result.0 as i32
                    ));
                }
            }
        }

        let cli_process = CliProcess::start()?;
        *guard = Some(cli_process);
    }

    Ok(CliProcessGuard(guard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn worker_parses_success_and_firmware_errors_without_retries() {
        let bytes =
            b"{\"code\":0,\"message\":\"[1,2]\"}\n{\"code\":1,\"message\":\"firmware refused\"}\n";
        let (requests, responses) = start_io_worker(Vec::<u8>::new(), Cursor::new(bytes));
        let first = exchange(&requests, &responses, "[]".into(), Duration::from_secs(1)).unwrap();
        assert_eq!(first.code, 0);
        let second = exchange(&requests, &responses, "[]".into(), Duration::from_secs(1)).unwrap();
        assert_eq!(second.code, 1);
        assert_eq!(second.message, "firmware refused");
    }

    #[test]
    fn worker_reports_eof_and_malformed_responses() {
        for bytes in [b"".as_slice(), b"not JSON\n".as_slice()] {
            let (requests, responses) = start_io_worker(Vec::<u8>::new(), Cursor::new(bytes));
            assert!(exchange(&requests, &responses, "[]".into(), Duration::from_secs(1)).is_err());
        }
    }

    #[test]
    fn response_wait_has_a_deadline_and_only_enqueues_once() {
        let (requests, queued) = mpsc::sync_channel(1);
        let (_sender, responses) = mpsc::sync_channel(1);
        let result = exchange(
            &requests,
            &responses,
            "firmware write".into(),
            Duration::from_millis(10),
        );
        assert!(result.unwrap_err().contains("CLI response unavailable"));
        assert_eq!(queued.try_recv().unwrap(), "firmware write");
        assert!(queued.try_recv().is_err());
    }
}
