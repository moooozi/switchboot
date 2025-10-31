use crate::types::CliCommand;
use std::process::Command;

pub fn call_cli(cmd: &CliCommand) -> Result<String, String> {
    let args = cmd.to_args();

    let executable_path = std::env::current_exe().map_err(|e| e.to_string())?;

    #[cfg(target_os = "linux")]
    let mut cmd = {
        if cmd.requires_root_privileges() {
            let mut c = Command::new("pkexec");
            // if the command is allowed to run without interactive auth, prefer
            // the nopass wrapper. Otherwise use the regular CLI binary.
            let mut p = executable_path.clone();
            p.set_file_name("switchboot-cli");
            c.arg(&p);
            c
        } else {
            let mut c = Command::new(&executable_path);
            c.arg("--cli");
            c
        }
    };

    #[cfg(not(target_os = "linux"))]
    let mut cmd = {
        let mut c = Command::new(&cli_path);
        c.arg("--cli");
        c
    };

    cmd.args(args);

    let output = cmd.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
