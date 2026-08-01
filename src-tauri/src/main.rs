// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod args_parser;

use switchboot_lib::cli::logic;

#[cfg(windows)]
use switchboot_lib::cli::windows;
#[cfg(windows)]
use switchboot_lib::constants::PIPE_SERVER_WAIT_TIMEOUT;

/// Entry point for the application.
/// Fast startup with minimal allocations - mode detection defers work until needed.
fn main() {
    let mut args = std::env::args();
    let argv0 = args.next().unwrap_or_default();

    // Detect mode with minimal overhead (busybox-style fast path)
    let mode = match args_parser::detect_mode(&argv0, &mut args) {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    // Execute based on detected mode
    match mode {
        args_parser::AppMode::Gui => {
            // GUI mode - check for root on Linux
            #[cfg(target_os = "linux")]
            {
                if unsafe { libc::geteuid() } == 0 {
                    eprintln!(
                        "Error: Running the GUI as root is not allowed for security reasons."
                    );
                    std::process::exit(1);
                }
            }

            // Launch Tauri GUI
            switchboot_lib::run(None);
        }

        args_parser::AppMode::CliDaemon => {
            logic::run_daemon();
        }

        args_parser::AppMode::CliCommand(args) => {
            std::process::exit(run_cli_command(args));
        }

        args_parser::AppMode::Exec {
            command,
            should_reboot,
        } => {
            if let Err(e) = args_parser::execute_command(&command, should_reboot) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
            std::process::exit(0);
        }

        #[cfg(windows)]
        args_parser::AppMode::WindowsService(service_arg) => {
            run_windows_service(&service_arg);
        }
    }
}

/// Execute CLI command mode - parse and run CliCommand
fn run_cli_command(args: Vec<String>) -> i32 {
    logic::run(args)
}

/// Execute Windows service command
#[cfg(windows)]
fn run_windows_service(service_arg: &str) {
    match service_arg {
        "/service_connector" => {
            windows::service::launch_windows_service_connector();
        }
        "/pipe_server" => {
            windows::pipe::run_unelevated_pipe_server(Some(PIPE_SERVER_WAIT_TIMEOUT), false);
        }
        "/pipe_server_test" => {
            windows::pipe::run_unelevated_pipe_server(None, true);
        }
        "/elevated_connector" => {
            windows::pipe::run_elevated_connector();
        }
        "/service_manager" => {
            windows::service::run_service_manager();
        }
        "/install_service" => {
            windows::service::install_service();
        }
        "/uninstall_service" => {
            windows::service::uninstall_service();
        }
        _ => {
            eprintln!(
                "Error: Unrecognized Windows service command '{}'.",
                service_arg
            );
            std::process::exit(1);
        }
    }
}
