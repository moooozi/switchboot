use switchboot_lib;

pub fn run(args: Vec<String>) -> i32 {
    eprintln!("[DEBUG] cli::run called with args: {:?}", args);
    match args.get(0).map(|s| s.as_str()) {
        Some("set-boot-order") => {
            let ids: Vec<u16> = args[1..].iter().filter_map(|s| s.parse().ok()).collect();
            match switchboot_lib::set_boot_order_internal(ids) {
                Ok(_) => 0,
                Err(e) => {
                    eprintln!("{}", e);
                    1
                }
            }
        }
        Some("set-boot-next") => {
            if let Some(id) = args.get(1).and_then(|s| s.parse().ok()) {
                match switchboot_lib::set_boot_next_internal(id) {
                    Ok(_) => 0,
                    Err(e) => {
                        eprintln!("{}", e);
                        1
                    }
                }
            } else {
                eprintln!("Missing entry id");
                1
            }
        }
        Some("save-boot-order") => {
            let ids: Vec<u16> = args[1..].iter().filter_map(|s| s.parse().ok()).collect();
            match switchboot_lib::save_boot_order_internal(ids) {
                Ok(_) => 0,
                Err(e) => {
                    eprintln!("{}", e);
                    1
                }
            }
        }
        Some("unset-boot-next") => {
            if args.len() == 1 {
                match switchboot_lib::unset_boot_next_internal() {
                    Ok(_) => 0,
                    Err(e) => {
                        eprintln!("{}", e);
                        1
                    }
                }
            } else {
                eprintln!("unset-boot-next takes no arguments");
                1
            }
        }
        // Add more CLI actions as needed
        _ => {
            eprintln!("Unknown or missing CLI action");
            1
        }
    }
}