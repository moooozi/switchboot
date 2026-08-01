use switchboot_lib::types::CliCommand;

#[derive(Debug)]
pub enum AppMode {
    Gui,
    CliDaemon,
    /// Unparsed args; CliCommand parsing is deferred until needed.
    CliCommand(Vec<String>),
    Exec {
        command: CliCommand,
        should_reboot: bool,
    },
    #[cfg(windows)]
    WindowsService(String),
}

/// Detect the execution mode from argv0 and the command-line arguments.
pub fn detect_mode(
    argv0: &str,
    args: &mut impl Iterator<Item = String>,
) -> Result<AppMode, String> {
    // Linux: a `switchboot-cli` symlink invocation is the optimized CLI route.
    #[cfg(target_os = "linux")]
    if argv0.ends_with("switchboot-cli") {
        let cli_args: Vec<String> = args.collect();
        if cli_args.len() == 1 && cli_args[0] == "--daemon" {
            return Ok(AppMode::CliDaemon);
        }
        return Ok(AppMode::CliCommand(cli_args));
    }

    let Some(first_arg) = args.next() else {
        return Ok(AppMode::Gui);
    };

    match first_arg.as_str() {
        "--cli" => {
            let cli_args: Vec<String> = args.collect();
            if cli_args.len() == 1 && cli_args[0] == "--daemon" {
                Ok(AppMode::CliDaemon)
            } else {
                Ok(AppMode::CliCommand(cli_args))
            }
        }

        "--exec" => {
            let remaining: Vec<String> = args.collect();
            if remaining.is_empty() {
                return Err("--exec requires a command".to_string());
            }

            let has_reboot = remaining.iter().any(|a| a == "reboot");
            let cmd_args: Vec<String> = remaining.into_iter().filter(|a| a != "reboot").collect();

            let command = CliCommand::from_args(&cmd_args)
                .map_err(|e| format!("Invalid --exec command: {}", e))?;

            if !command.allow_non_interactive_exec() {
                return Err(format!(
                    "Command '{}' is not allowed in --exec mode",
                    cmd_args[0]
                ));
            }

            Ok(AppMode::Exec {
                command,
                should_reboot: has_reboot,
            })
        }

        #[cfg(windows)]
        arg if arg.starts_with('/') => Ok(AppMode::WindowsService(arg.to_string())),

        _ => Ok(AppMode::Gui),
    }
}

/// Execute a command in exec mode with optional reboot (command is pre-validated).
pub fn execute_command(command: &CliCommand, should_reboot: bool) -> Result<(), String> {
    match command {
        CliCommand::SetBootNext(entry_id) => {
            switchboot_lib::handle_bootnext_shortcut_execution(*entry_id, should_reboot)
        }
        CliCommand::SetBootFirmware => {
            switchboot_lib::handle_bootfw_shortcut_execution(should_reboot)
        }
        CliCommand::UnsetBootFirmware => {
            switchboot_lib::handle_bootfw_shortcut_execution(should_reboot)
        }
        _ => Err(format!(
            "Command {:?} is not supported in exec mode",
            command
        )),
    }
}
