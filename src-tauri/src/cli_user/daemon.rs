use crate::types::CommandResponse;
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, OnceLock};

static CLI_PROCESS: OnceLock<Mutex<CliProcess>> = OnceLock::new();

pub struct CliProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl CliProcess {
    fn start() -> Result<Self, String> {
        let cli_path = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .map(|p| p.join("switchboot-cli"))
            .ok_or("Failed to find CLI binary")?;

        #[cfg(target_os = "linux")]
        let mut cmd = {
            let mut c = Command::new("pkexec");
            c.arg(&cli_path);
            c.arg("--daemon");
            c
        };

        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut c = Command::new(&cli_path);
            c.arg("/service_client");
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
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    pub fn send_command<T: for<'a> Deserialize<'a>>(&mut self, args: &[&str]) -> Result<T, String> {
        let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let cmd_json = serde_json::to_string(&args_vec).map_err(|e| e.to_string())?;
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

    pub fn send_command_unit(&mut self, args: &[&str]) -> Result<(), String> {
        let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let cmd_json = serde_json::to_string(&args_vec).map_err(|e| e.to_string())?;
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
pub fn get_cli() -> Result<std::sync::MutexGuard<'static, CliProcess>, String> {
    CLI_PROCESS
        .get_or_init(|| {
            CliProcess::start()
                .map(Mutex::new)
                .unwrap_or_else(|e| panic!("Failed to start CLI process: {}", e))
        })
        .lock()
        .map_err(|_| "Failed to lock CLI process".to_string())
}