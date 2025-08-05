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

    // --- Handle shortcut execution ---
    let mut args = std::env::args().skip(1).peekable();
    if let Some(arg) = args.peek() {
        if arg == "--set-boot-next" {
            args.next(); // consume the flag
            if let Some(entry_id_str) = args.next() {
                match entry_id_str.parse::<u16>() {
                    Ok(entry_id) => {
                        let should_reboot = args.any(|a| a == "--reboot");
                        #[cfg(target_os = "windows")]
                        {
                            if let Err(e) = switchboot_lib::handle_bootnext_shortcut_execution(
                                entry_id,
                                should_reboot,
                            ) {
                                eprintln!("Error: {e}");
                                std::process::exit(1);
                            }
                            std::process::exit(0);
                        }
                    }
                    Err(_) => {
                        eprintln!("Error: Invalid entry ID: {entry_id_str}");
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error: Missing entry ID after --set-boot-next");
                std::process::exit(1);
            }
        }
    }

    // Launch Tauri GUI
    switchboot_lib::run();
}
