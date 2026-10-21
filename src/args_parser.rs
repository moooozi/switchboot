use switchboot_core::types::CliCommand;

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
    #[cfg(not(target_os = "linux"))]
    let _ = argv0;

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
            #[cfg(windows)]
            if let Some(arg) = cli_args.first().filter(|arg| arg.starts_with('/')) {
                return Ok(AppMode::WindowsService(arg.clone()));
            }
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

            let mut cmd_args = remaining;
            let has_reboot = cmd_args.last().is_some_and(|arg| arg == "reboot");
            if has_reboot {
                cmd_args.pop();
            }
            if cmd_args.is_empty() {
                return Err("--exec requires a command".to_string());
            }

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
            switchboot_core::handle_bootnext_shortcut_execution(*entry_id, should_reboot)
        }
        CliCommand::SetBootFirmware => {
            switchboot_core::handle_bootfw_shortcut_execution(true, should_reboot)
        }
        CliCommand::UnsetBootFirmware => {
            switchboot_core::handle_bootfw_shortcut_execution(false, should_reboot)
        }
        _ => Err(format!(
            "Command {:?} is not supported in exec mode",
            command
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(args: &[&str]) -> Result<AppMode, String> {
        detect_mode("switchboot", &mut args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn ipc_build_version_is_a_version_not_a_config_path() {
        use switchboot_core::build_info::{APP_IDENTIFIER, APP_IDENTIFIER_VERSION, APP_VERSION};
        assert_eq!(APP_VERSION, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            APP_IDENTIFIER_VERSION,
            format!("{APP_IDENTIFIER}-{}", env!("CARGO_PKG_VERSION"))
        );
        assert!(!APP_IDENTIFIER_VERSION.contains(['/', '\\']));
    }

    #[test]
    fn gui_is_the_default() {
        assert!(matches!(detect(&[]).unwrap(), AppMode::Gui));
        assert!(matches!(detect(&["unknown"]).unwrap(), AppMode::Gui));
    }

    #[test]
    fn cli_commands_and_daemon_are_preserved() {
        assert!(matches!(
            detect(&["--cli", "--daemon"]).unwrap(),
            AppMode::CliDaemon
        ));
        let AppMode::CliCommand(args) = detect(&["--cli", "get-boot-entries"]).unwrap() else {
            panic!("expected CLI command");
        };
        assert_eq!(args, ["get-boot-entries"]);
    }

    #[test]
    fn shortcuts_parse_without_executing() {
        for (args, expected_reboot) in [
            (vec!["--exec", "set-boot-next", "42"], false),
            (vec!["--exec", "set-boot-next", "42", "reboot"], true),
        ] {
            assert!(matches!(detect(&args).unwrap(), AppMode::Exec {
                command: CliCommand::SetBootNext(42), should_reboot
            } if should_reboot == expected_reboot));
        }
        assert!(matches!(
            detect(&["--exec", "set-boot-fw", "reboot"]).unwrap(),
            AppMode::Exec {
                command: CliCommand::SetBootFirmware,
                should_reboot: true
            }
        ));
    }

    #[test]
    fn invalid_exec_arguments_return_errors() {
        for args in [
            vec!["--exec"],
            vec!["--exec", "reboot"],
            vec!["--exec", "set-boot-next"],
            vec!["--exec", "set-boot-next", "65536"],
            vec!["--exec", "get-boot-order"],
            vec!["--exec", "get-boot-fw"],
            vec!["--exec", "set-boot-fw", "extra"],
            vec!["--exec", "set-boot-next", "42", "43"],
            vec!["--exec", "reboot", "set-boot-fw"],
            vec!["--exec", "set-boot-fw", "reboot", "reboot"],
            vec!["--exec", "unknown"],
        ] {
            assert!(detect(&args).is_err(), "unexpected success for {args:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_service_routes_work_with_and_without_cli_prefix() {
        for service_arg in [
            "/service_connector",
            "/pipe_server",
            "/pipe_server_test",
            "/elevated_connector",
            "/service_manager",
            "/install_service",
            "/uninstall_service",
        ] {
            for args in [vec![service_arg], vec!["--cli", service_arg]] {
                assert!(
                    matches!(detect(&args).unwrap(), AppMode::WindowsService(arg) if arg == service_arg)
                );
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_symlink_routes_commands_and_daemon() {
        let mut args = ["get-boot-entries".to_string()].into_iter();
        assert!(
            matches!(detect_mode("/usr/bin/switchboot-cli", &mut args).unwrap(),
            AppMode::CliCommand(args) if args == ["get-boot-entries"])
        );
        let mut args = ["--daemon".to_string()].into_iter();
        assert!(matches!(
            detect_mode("/usr/bin/switchboot-cli", &mut args).unwrap(),
            AppMode::CliDaemon
        ));
    }
}
