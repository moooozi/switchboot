use super::{BootEntry, CommandResponse};
use firmware_variables::{boot, privileges};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CliCommand {
    SetBootOrder(Vec<String>),
    SetBootNext(Option<String>),
    SaveBootOrder(Vec<String>),
    UnsetBootNext,
    GetBootOrder,
    GetBootNext,
    GetBootEntries,
    GetBootCurrent,
    Unknown,
}

impl CliCommand {
    pub fn from_args(args: &[String]) -> Self {
        match args.get(0).map(String::as_str) {
            Some("set-boot-order") => CliCommand::SetBootOrder(args[1..].to_vec()),
            Some("set-boot-next") => CliCommand::SetBootNext(args.get(1).cloned()),
            Some("save-boot-order") => CliCommand::SaveBootOrder(args[1..].to_vec()),
            Some("unset-boot-next") => CliCommand::UnsetBootNext,
            Some("get-boot-order") => CliCommand::GetBootOrder,
            Some("get-boot-next") => CliCommand::GetBootNext,
            Some("get-boot-entries") => CliCommand::GetBootEntries,
            Some("get-boot-current") => CliCommand::GetBootCurrent,
            _ => CliCommand::Unknown,
        }
    }
}

pub fn dispatch_command(command: CliCommand) -> CommandResponse {
    match command {
        CliCommand::SetBootOrder(ids) => set_boot_order_response(&ids),
        CliCommand::SetBootNext(id) => set_boot_next_response(id.as_ref()),
        CliCommand::SaveBootOrder(ids) => save_boot_order_response(&ids),
        CliCommand::UnsetBootNext => unset_boot_next_response(),
        CliCommand::GetBootOrder => get_boot_order_response(),
        CliCommand::GetBootNext => get_boot_next_response(),
        CliCommand::GetBootEntries => get_boot_entries_response(),
        CliCommand::GetBootCurrent => get_boot_current_response(),
        CliCommand::Unknown => CommandResponse {
            code: 1,
            message: "Unknown or missing CLI action".to_string(),
        },
    }
}

fn with_privileges<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, Box<dyn std::error::Error>>,
{
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    f().map_err(|e| e.to_string())
}

pub fn run_daemon() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let args: Vec<String> = match serde_json::from_str(&line) {
            Ok(a) => a,
            Err(e) => {
                let resp = CommandResponse {
                    code: 1,
                    message: format!("Invalid command format: {}", e),
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        let command = CliCommand::from_args(&args);
        let response = dispatch_command(command);
        let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
        let _ = stdout.flush();
    }
}

/// Runs the CLI interface for switchboot.
/// Returns 0 on success, 1 on error.
pub fn run(args: Vec<String>) -> i32 {
    eprintln!("[DEBUG] cli::run called with args: {:?}", args);

    let command = CliCommand::from_args(&args);
    let response = dispatch_command(command);
    if response.code == 0 {
        println!("{}", response.message);
    } else {
        eprintln!("{}", response.message);
    }
    response.code
}

fn set_boot_order_response(ids: &[String]) -> CommandResponse {
    let parsed_ids: Vec<u16> = ids.iter().filter_map(|s| s.parse().ok()).collect();
    match with_privileges(|| boot::set_boot_order(&parsed_ids)) {
        Ok(_) => CommandResponse {
            code: 0,
            message: "Boot order set successfully".to_string(),
        },
        Err(e) => CommandResponse {
            code: 1,
            message: format!("Error setting boot order: {}", e),
        },
    }
}

fn set_boot_next_response(id_arg: Option<&String>) -> CommandResponse {
    match id_arg.and_then(|s| s.parse().ok()) {
        Some(id) => match with_privileges(|| boot::set_boot_next(id)) {
            Ok(_) => CommandResponse {
                code: 0,
                message: "Boot next set successfully".to_string(),
            },
            Err(e) => CommandResponse {
                code: 1,
                message: format!("Error setting boot next: {}", e),
            },
        },
        None => CommandResponse {
            code: 1,
            message: "Missing or invalid entry id for set-boot-next".to_string(),
        },
    }
}

fn save_boot_order_response(ids: &[String]) -> CommandResponse {
    let parsed_ids: Vec<u16> = ids.iter().filter_map(|s| s.parse().ok()).collect();
    match with_privileges(|| {
        boot::set_boot_order(&parsed_ids)?;
        if let Some(&first_entry) = parsed_ids.first() {
            boot::set_boot_next(first_entry)?;
        }
        Ok(())
    }) {
        Ok(_) => CommandResponse {
            code: 0,
            message: "Boot order saved successfully".to_string(),
        },
        Err(e) => CommandResponse {
            code: 1,
            message: format!("Error saving boot order: {}", e),
        },
    }
}

fn unset_boot_next_response() -> CommandResponse {
    match with_privileges(boot::unset_boot_next) {
        Ok(_) => CommandResponse {
            code: 0,
            message: "Boot next unset successfully".to_string(),
        },
        Err(e) => CommandResponse {
            code: 1,
            message: format!("Error unsetting boot next: {}", e),
        },
    }
}

fn get_boot_order_response() -> CommandResponse {
    match with_privileges(boot::get_boot_order) {
        Ok(order) => CommandResponse {
            code: 0,
            message: serde_json::to_string(&order).unwrap(),
        },
        Err(e) => CommandResponse {
            code: 1,
            message: format!("Error getting boot order: {}", e),
        },
    }
}

fn get_boot_next_response() -> CommandResponse {
    match with_privileges(boot::get_boot_next) {
        Ok(order) => CommandResponse {
            code: 0,
            message: serde_json::to_string(&order).unwrap(),
        },
        Err(e) => CommandResponse {
            code: 1,
            message: format!("Error getting boot next: {}", e),
        },
    }
}

fn get_boot_entries_response() -> CommandResponse {
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
        Ok(entries) => CommandResponse {
            code: 0,
            message: serde_json::to_string(&entries).unwrap(),
        },
        Err(e) => CommandResponse {
            code: 1,
            message: format!("Error getting boot entries: {}", e),
        },
    }
}

fn get_boot_current_response() -> CommandResponse {
    match with_privileges(boot::get_boot_current) {
        Ok(entry) => CommandResponse {
            code: 0,
            message: serde_json::to_string(&entry).unwrap(),
        },
        Err(e) => CommandResponse {
            code: 1,
            message: format!("Error getting boot current: {}", e),
        },
    }
}
