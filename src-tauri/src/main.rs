// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod args_parser;
mod build_info;
mod cli;
mod types;

#[cfg(windows)]
pub use cli::windows;

pub use cli::logic;

pub const PIPE_SERVER_WAIT_TIMEOUT: u64 = 5; // 5 seconds

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
                "/service" => {
                    windows::service::launch_windows_service();
                    return;
                }
                "/pipe_server" => {
                    windows::pipe::run_pipe_server(Some(PIPE_SERVER_WAIT_TIMEOUT), false);
                    return;
                }
                "/pipe_server_test" => {
                    windows::pipe::run_pipe_server(None, true);
                    return;
                }
                "/pipe_client" => {
                    windows::pipe::run_pipe_client();
                    return;
                }
                "/service_client" => {
                    windows::service::run_service_client();
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
                _ => {}
            }
        }
    }

    std::process::exit(logic::run(args));
}
