// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;

/// Entry point for the application.
/// Launches either the CLI or the Tauri GUI depending on arguments.
fn main() {
    let mut args = std::env::args();
    let _exe = args.next();

    match args.next().as_deref() {
        Some("--cli") => {
            let cli_args: Vec<String> = args.collect();
            std::process::exit(cli::run(cli_args));
        }
        _ => {
            #[cfg(target_os = "linux")]
            {
                if unsafe { libc::geteuid() } == 0 {
                    eprintln!("Error: Running the GUI as root is not allowed for security reasons.");
                    std::process::exit(1);
                }
            }
            // Launch Tauri GUI
            switchboot_lib::run();
        }
    }
}
