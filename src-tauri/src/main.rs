// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod args_parser;

use switchboot_lib::cli::logic;
use switchboot_lib::constants::PIPE_SERVER_WAIT_TIMEOUT;

#[cfg(windows)]
use switchboot_lib::cli::windows;

/// Entry point for the application.
/// Handles both GUI and CLI modes.
fn main() {
    let mut args = std::env::args();
    let _exe = args.next();
    let rest: Vec<String> = args.collect();

    // Parse command line arguments
    match args_parser::parse_args(rest.into_iter()) {
        Ok(config) => {
            match config.mode {
                args_parser::AppMode::Cli { args } => {
                    run_cli_mode(args);
                }
                args_parser::AppMode::Exec {
                    command,
                    should_reboot,
                } => {
                    if let Err(e) = args_parser::handle_exec_mode(&command, should_reboot) {
                        eprintln!("Error: {e}");
                        std::process::exit(1);
                    }
                    std::process::exit(0);
                }
                args_parser::AppMode::Gui => {
                    // GUI mode - check for root on Linux
                    #[cfg(target_os = "linux")]
                    {
                        if unsafe { libc::geteuid() } == 0 {
                            eprintln!("Error: Running the GUI as root is not allowed for security reasons.");
                            std::process::exit(1);
                        }
                    }

                    // Launch Tauri GUI
                    switchboot_lib::run(None);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

/// Handle CLI mode execution
fn run_cli_mode(args: Vec<String>) {
    // Handle --daemon on all platforms
    if args.len() == 1 && args[0] == "--daemon" {
        logic::run_daemon();
        return;
    }

    #[cfg(windows)]
    {
        if args.len() == 1 && args[0].starts_with('/') {
            match args[0].as_str() {
                "/service_connector" => {
                    windows::service::launch_windows_service_connector();
                    return;
                }
                "/pipe_server" => {
                    // Unelevated instance creates the pipe server
                    windows::pipe::run_unelevated_pipe_server(
                        Some(PIPE_SERVER_WAIT_TIMEOUT),
                        false,
                    );
                    return;
                }
                "/pipe_server_test" => {
                    windows::pipe::run_unelevated_pipe_server(None, true);
                    return;
                }
                "/elevated_connector" => {
                    // Elevated instance connects to the unelevated pipe server
                    windows::pipe::run_elevated_connector();
                    return;
                }
                "/service_manager" => {
                    // Unelevated instance that starts service and creates pipe server
                    windows::service::run_service_manager();
                    return;
                }
                "/install_service" => {
                    windows::service::install_service();
                    return;
                }
                "/uninstall_service" => {
                    windows::service::uninstall_service();
                    return;
                }
                _ => {
                    eprintln!("Error: Unrecognized command '{}'.", args[0]);
                    std::process::exit(1);
                }
            }
        }
    }

    std::process::exit(logic::run(args));
}
