use firmware_variables::{boot, privileges};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct BootEntry {
    pub id: u16,
    pub description: String,
    pub is_default: bool,
    pub is_bootnext: bool,
    pub is_current: bool,
}

fn with_privileges<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, Box<dyn std::error::Error>>,
{
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    f().map_err(|e| e.to_string())
}

fn main() {
    let mut args = std::env::args();
    let _exe = args.next();
    std::process::exit(run(args.collect()));
}

/// Runs the CLI interface for switchboot.
/// Returns 0 on success, 1 on error.
pub fn run(args: Vec<String>) -> i32 {
    eprintln!("[DEBUG] cli::run called with args: {:?}", args);

    match args.get(0).map(String::as_str) {
        Some("set-boot-order") => handle_set_boot_order(&args[1..]),
        Some("set-boot-next") => handle_set_boot_next(args.get(1)),
        Some("save-boot-order") => handle_save_boot_order(&args[1..]),
        Some("unset-boot-next") => handle_unset_boot_next(&args[1..]),
        Some("get-boot-order") => handle_get_boot_order(),
        Some("get-boot-next") => handle_get_boot_next(),
        Some("get-boot-entries") => handle_get_boot_entries(),
        Some("get-boot-current") => handle_get_boot_current(),
        _ => {
            eprintln!("Unknown or missing CLI action");
            1
        }
    }
}

fn handle_set_boot_order(ids: &[String]) -> i32 {
    let parsed_ids: Vec<u16> = ids.iter().filter_map(|s| s.parse().ok()).collect();
    match with_privileges(|| boot::set_boot_order(&parsed_ids)) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Error setting boot order: {}", e);
            1
        }
    }
}

fn handle_set_boot_next(id_arg: Option<&String>) -> i32 {
    match id_arg.and_then(|s| s.parse().ok()) {
        Some(id) => match with_privileges(|| boot::set_boot_next(id)) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("Error setting boot next: {}", e);
                1
            }
        },
        None => {
            eprintln!("Missing or invalid entry id for set-boot-next");
            1
        }
    }
}

fn handle_save_boot_order(ids: &[String]) -> i32 {
    let parsed_ids: Vec<u16> = ids.iter().filter_map(|s| s.parse().ok()).collect();
    match with_privileges(|| {
        boot::set_boot_order(&parsed_ids)?;
        if let Some(&first_entry) = parsed_ids.first() {
            boot::set_boot_next(first_entry)?;
        }
        Ok(())
    }) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Error saving boot order: {}", e);
            1
        }
    }
}

fn handle_unset_boot_next(args: &[String]) -> i32 {
    if args.is_empty() {
        match with_privileges(boot::unset_boot_next) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("Error unsetting boot next: {}", e);
                1
            }
        }
    } else {
        eprintln!("unset-boot-next takes no arguments");
        1
    }
}

fn handle_get_boot_order() -> i32 {
    match with_privileges(boot::get_boot_order) {
        Ok(order) => {
            println!("{}", serde_json::to_string(&order).unwrap());
            0
        }
        Err(e) => {
            eprintln!("Error getting boot order: {}", e);
            1
        }
    }
}

fn handle_get_boot_next() -> i32 {
    match with_privileges(boot::get_boot_next) {
        Ok(order) => {
            println!("{}", serde_json::to_string(&order).unwrap());
            0
        }
        Err(e) => {
            eprintln!("Error getting boot next: {}", e);
            1
        }
    }
}

fn handle_get_boot_entries() -> i32 {
    match with_privileges(|| {
        let boot_order = boot::get_boot_order()?;
        let boot_next = boot::get_boot_next()?;
        let boot_current = boot::get_boot_current()?;
        let mut entries = Vec::new();
        for (idx, &entry_id) in boot_order.iter().enumerate() {
            let parsed = boot::get_parsed_boot_entry(entry_id)?;
            entries.push(BootEntry {
                id: entry_id,
                description: parsed.description,
                is_default: idx == 0,
                is_bootnext: boot_next == Some(entry_id) && idx != 0,
                is_current: boot_current == Some(entry_id),
            });
        }
        Ok(entries)
    }) {
        Ok(entries) => {
            println!("{}", serde_json::to_string(&entries).unwrap());
            0
        }
        Err(e) => {
            eprintln!("Error getting boot entries: {}", e);
            1
        }
    }
}

fn handle_get_boot_current() -> i32 {
    match with_privileges(boot::get_boot_current) {
        Ok(entry) => {
            println!("{}", serde_json::to_string(&entry).unwrap());
            0
        }
        Err(e) => {
            eprintln!("Error getting boot current: {}", e);
            1
        }
    }
}