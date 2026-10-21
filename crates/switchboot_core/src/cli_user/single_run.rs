use crate::types::CliCommand;
use std::process::Command;

pub fn call_cli(cmd: &CliCommand) -> Result<String, String> {
    let args = cmd.to_args();

    let executable_path = std::env::current_exe().map_err(|e| e.to_string())?;

    // Platform-specific CLI invocation
    #[cfg(target_os = "linux")]
    let mut cmd = {
        if cmd.requires_root_privileges() {
            // Privileged: Use pkexec with switchboot-cli symlink
            // The symlink is detected via argv0, no --cli flag needed
            let mut c = Command::new("pkexec");
            let mut p = executable_path.clone();
            p.set_file_name("switchboot-cli");
            c.arg(&p);
            c
        } else {
            // Non-privileged: Direct execution with --cli flag
            let mut c = Command::new(&executable_path);
            c.arg("--cli");
            c
        }
    };

    #[cfg(target_os = "windows")]
    let mut cmd = {
        // Windows: Always use --cli flag (no symlink support)
        let mut c = Command::new(&executable_path);
        c.arg("--cli");
        c
    };

    cmd.args(args);

    let output = cmd.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else if output.status.code() == Some(127) {
        Err("Authentication was cancelled or denied.".to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
