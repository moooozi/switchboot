// Example: Print boot order and descriptions using firmware_variables
fn main() {
    // Set privilege guard (Windows only)
    #[cfg(windows)]
    let _priv_guard = match firmware_variables::privileges::windows::adjust_privileges() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Failed to adjust privileges: {}", e);
            return;
        }
    };

    // Get boot order
    match firmware_variables::boot::get_boot_order() {
        Ok(order) => {
            for entry_id in order {
                match firmware_variables::boot::get_parsed_boot_entry(entry_id) {
                    Ok(load_option) => {
                        println!("{}", load_option.description);
                    }
                    Err(e) => {
                        // Try to get raw bytes for debugging
                        match firmware_variables::boot::get_boot_entry(entry_id) {
                            Ok(raw) => {
                                eprintln!(
                                    "Failed to parse Boot{:04X}: {}\nRaw: {}",
                                    entry_id,
                                    e,
                                    hex_fmt(&raw)
                                );
                            }
                            Err(e2) => {
                                eprintln!(
                                    "Failed to parse Boot{:04X}: {} (and failed to get raw: {})",
                                    entry_id, e, e2
                                );
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to get boot order: {}", e);
        }
    }
}

fn hex_fmt(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
