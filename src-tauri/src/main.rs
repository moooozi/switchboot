// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod args_parser;


/// Entry point for the application.
/// Launches the Tauri GUI.
fn main() {
    #[cfg(target_os = "linux")]
    {
        if unsafe { libc::geteuid() } == 0 {
            eprintln!("Error: Running the GUI as root is not allowed for security reasons.");
            std::process::exit(1);
        }
    }

    // Parse command line arguments
    match args_parser::parse_args(std::env::args().skip(1)) {
        Ok(config) => match config.mode {
            args_parser::AppMode::Exec { command, should_reboot } => {
                if let Err(e) = args_parser::handle_exec_mode(&command, should_reboot) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
                std::process::exit(0);
            }
            args_parser::AppMode::Gui => {
                // Create config and launch Tauri GUI
                #[cfg(target_os = "windows")]
                let app_config = Some(switchboot_lib::config::AppConfig {
                    portable_mode: config.portable_mode,
                });
                
                #[cfg(not(target_os = "windows"))]
                let app_config = None;
                
                switchboot_lib::run(app_config);
            }
        },
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
