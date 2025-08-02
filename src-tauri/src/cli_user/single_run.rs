use crate::types::CliCommand;
use std::process::Command;

pub fn call_cli(cmd: &CliCommand, needs_privilege: bool) -> Result<String, String> {
    let args = cmd.to_args();

    let cli_path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|p| p.join("switchboot-cli"))
        .ok_or("Failed to find CLI binary")?;

    #[cfg(target_os = "linux")]
    let mut cmd = {
        if needs_privilege {
            let mut c = Command::new("pkexec");
            c.arg(&cli_path);
            c
        } else {
            Command::new(&cli_path)
        }
    };

    #[cfg(not(target_os = "linux"))]
    let mut cmd = Command::new(&cli_path);

    cmd.args(args);

    let output = cmd.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
