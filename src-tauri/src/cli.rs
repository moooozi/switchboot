use switchboot_lib;

/// Runs the CLI interface for switchboot.
/// Returns 0 on success, 1 on error.
pub fn run(args: Vec<String>) -> i32 {
    eprintln!("[DEBUG] cli::run called with args: {:?}", args);

    match args.get(0).map(String::as_str) {
        Some("set-boot-order") => handle_set_boot_order(&args[1..]),
        Some("set-boot-next") => handle_set_boot_next(args.get(1)),
        Some("save-boot-order") => handle_save_boot_order(&args[1..]),
        Some("unset-boot-next") => handle_unset_boot_next(&args[1..]),
        _ => {
            eprintln!("Unknown or missing CLI action");
            1
        }
    }
}

fn handle_set_boot_order(ids: &[String]) -> i32 {
    let parsed_ids: Vec<u16> = ids.iter().filter_map(|s| s.parse().ok()).collect();
    match switchboot_lib::set_boot_order_internal(parsed_ids) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Error setting boot order: {}", e);
            1
        }
    }
}

fn handle_set_boot_next(id_arg: Option<&String>) -> i32 {
    match id_arg.and_then(|s| s.parse().ok()) {
        Some(id) => match switchboot_lib::set_boot_next_internal(id) {
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
    match switchboot_lib::save_boot_order_internal(parsed_ids) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Error saving boot order: {}", e);
            1
        }
    }
}

fn handle_unset_boot_next(args: &[String]) -> i32 {
    if args.is_empty() {
        match switchboot_lib::unset_boot_next_internal() {
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