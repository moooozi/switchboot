// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod args_parser;
mod backend;

use switchboot_core::cli::logic;

#[cfg(windows)]
use switchboot_core::cli::windows;
#[cfg(windows)]
use switchboot_core::constants::PIPE_SERVER_WAIT_TIMEOUT;

fn main() {
    // Demo mode never opens a firmware transport or requests elevation.
    if std::env::args().skip(1).eq(["--demo".to_owned()]) {
        run_ui(std::sync::Arc::new(switchboot_ui::DemoBackend::default()));
        return;
    }
    let mut args = std::env::args();
    let argv0 = args.next().unwrap_or_default();

    let mode = match args_parser::detect_mode(&argv0, &mut args) {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    match mode {
        args_parser::AppMode::Gui => {
            #[cfg(target_os = "linux")]
            {
                if unsafe { libc::geteuid() } == 0 {
                    eprintln!(
                        "Error: Running the GUI as root is not allowed for security reasons."
                    );
                    std::process::exit(1);
                }
            }

            run_ui(std::sync::Arc::new(backend::FirmwareBackend));
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

fn run_ui(backend: std::sync::Arc<dyn switchboot_ui::Backend>) {
    if let Err(error) = switchboot_ui::run(backend) {
        eprintln!("Failed to start Switchboot: {error}");
        std::process::exit(1);
    }
}

fn run_cli_command(args: Vec<String>) -> i32 {
    logic::run(args)
}

#[cfg(windows)]
fn run_windows_service(service_arg: &str) {
    match service_arg {
        "/service_connector" => {
            windows::service::launch_windows_service_connector();
        }
        "/pipe_server" => {
            if let Err(error) =
                windows::pipe::run_unelevated_pipe_server(Some(PIPE_SERVER_WAIT_TIMEOUT), false)
            {
                eprintln!("Pipe server failed: {error}");
                std::process::exit(1);
            }
        }
        "/pipe_server_test" => {
            if let Err(error) = windows::pipe::run_unelevated_pipe_server(None, true) {
                eprintln!("Pipe server failed: {error}");
                std::process::exit(1);
            }
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
