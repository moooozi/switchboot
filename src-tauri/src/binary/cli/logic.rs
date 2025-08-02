use crate::types::{BootEntry, CliCommand, CommandResponse};
use firmware_variables::{boot, privileges};
use std::io::{BufRead, BufReader, Write};

pub fn dispatch_command(command: CliCommand) -> CommandResponse {
    match command {
        CliCommand::SetBootOrder(ids) => set_boot_order_response(&ids),
        CliCommand::SetBootNext(id) => set_boot_next_response(Some(id)),
        CliCommand::SaveBootOrder(ids) => save_boot_order_response(&ids),
        CliCommand::UnsetBootNext => unset_boot_next_response(),
        CliCommand::GetBootOrder => get_boot_order_response(),
        CliCommand::GetBootNext => get_boot_next_response(),
        CliCommand::GetBootEntries => get_boot_entries_response(),
        CliCommand::GetBootCurrent => get_boot_current_response(),
        CliCommand::RestartNow => CommandResponse {
            code: 1,
            message: "Restart not supported in CLI".to_string(),
        },
    }
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

        let command = match CliCommand::from_args(&args) {
            Ok(cmd) => cmd,
            Err(e) => {
                let resp = CommandResponse {
                    code: 1,
                    message: e,
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };
        let response = dispatch_command(command);
        let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
        let _ = stdout.flush();
    }
}

/// Runs the CLI interface for switchboot.
/// Returns 0 on success, 1 on error.
pub fn run(args: Vec<String>) -> i32 {
    eprintln!("[DEBUG] cli::run called with args: {:?}", args);

    let command = match CliCommand::from_args(&args) {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    let response = dispatch_command(command);
    if response.code == 0 {
        println!("{}", response.message);
    } else {
        eprintln!("{}", response.message);
    }
    response.code
}

fn set_boot_order_response(ids: &Vec<u16>) -> CommandResponse {
    match with_privileges(|| boot::set_boot_order(ids)) {
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

fn set_boot_next_response(id: Option<u16>) -> CommandResponse {
    match id {
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

fn save_boot_order_response(ids: &Vec<u16>) -> CommandResponse {
    match with_privileges(|| {
        boot::set_boot_order(ids)?;
        if let Some(&first_entry) = ids.first() {
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

fn with_privileges<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, Box<dyn std::error::Error>>,
{
    let _guard = privileges::adjust_privileges().map_err(|e| e.to_string())?;
    f().map_err(|e| e.to_string())
}
