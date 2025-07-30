// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;

fn main() {
    let mut args = std::env::args();
    let _exe = args.next();
    match args.next().as_deref() {
        Some("--cli") => {
            // Debug: print CLI args
            let collected: Vec<String> = args.collect();
            eprintln!("[DEBUG] Entering CLI mode with args: {:?}", collected);
            cli::run(collected);
        }
        _ => {
            // Launch Tauri GUI
            switchboot_lib::run()
        }
    }
}
