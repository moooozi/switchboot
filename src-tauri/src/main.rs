// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


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
    // Launch Tauri GUI
    switchboot_lib::run();
}
