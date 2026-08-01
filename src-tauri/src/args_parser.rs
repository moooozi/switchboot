use switchboot_lib::types::CliCommand;

/// Application execution mode determined from argv0 and command line arguments.
/// Optimized for fast startup with minimal allocations.
#[derive(Debug)]
pub enum AppMode {
    /// Launch the GUI (default mode, zero allocations)
    Gui,
    /// Run CLI daemon mode (--daemon flag)
    CliDaemon,
    /// Execute CLI commands (stores unparsed args for deferred CliCommand parsing)
    CliCommand(Vec<String>),
    /// Execute a command and optionally reboot (--exec mode)
    Exec {
        command: CliCommand,
        should_reboot: bool,
    },
    #[cfg(windows)]
    /// Windows-specific service commands (/service_connector, /pipe_server, etc.)
    WindowsService(String),
}

/// Detect application mode from argv0 and arguments with minimal overhead.
/// Uses busybox-style single-pass parsing for fast startup.
///
/// # Arguments
/// * `argv0` - Program name (argv[0]) - used to detect switchboot-cli symlink on Linux
/// * `args` - Mutable iterator over arguments (will be consumed as needed)
///
/// # Performance
/// - GUI mode: Zero allocations (most common case)
/// - CLI mode: Single Vec allocation, deferred command parsing
/// - Exec mode: Single Vec allocation, immediate command parsing
pub fn detect_mode(
    argv0: &str,
    args: &mut impl Iterator<Item = String>,
) -> Result<AppMode, String> {
    // Linux-only: Check for switchboot-cli symlink invocation
    #[cfg(target_os = "linux")]
    if argv0.ends_with("switchboot-cli") {
        let cli_args: Vec<String> = args.collect();
        // Special case: --daemon flag
        if cli_args.len() == 1 && cli_args[0] == "--daemon" {
            return Ok(AppMode::CliDaemon);
        }
        return Ok(AppMode::CliCommand(cli_args));
    }

    // Peek at first argument to determine mode
    let Some(first_arg) = args.next() else {
        return Ok(AppMode::Gui); // No args = GUI mode
    };

    match first_arg.as_str() {
        // Explicit --cli flag (symlink argv0 path is the optimized Linux route,
        // but --cli is still used by the daemon and non-privileged single-run
        // invocations on all platforms, and for manual use)
        "--cli" => {
            let cli_args: Vec<String> = args.collect();
            if cli_args.len() == 1 && cli_args[0] == "--daemon" {
                Ok(AppMode::CliDaemon)
            } else {
                Ok(AppMode::CliCommand(cli_args))
            }
        }

        // Exec mode: Parse command immediately for validation
        "--exec" => {
            let remaining: Vec<String> = args.collect();
            if remaining.is_empty() {
                return Err("--exec requires a command".to_string());
            }

            // Separate reboot flag from command args
            let has_reboot = remaining.iter().any(|a| a == "reboot");
            let cmd_args: Vec<String> = remaining.into_iter().filter(|a| a != "reboot").collect();

            // Parse and validate command
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

        // Windows-only: Service commands (/service_connector, /pipe_server, etc.)
        #[cfg(windows)]
        arg if arg.starts_with('/') => Ok(AppMode::WindowsService(arg.to_string())),

        // Any other argument = GUI mode (ignore unknown flags)
        _ => Ok(AppMode::Gui),
    }
}

/// Execute a command in exec mode with optional reboot.
/// Only allowed commands are executed (validated during parsing).
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
