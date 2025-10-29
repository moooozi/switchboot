// Example: Discover EFI boot entries in a range using firmware_variables
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

    // Discover and parse boot entries in range 0x0000 to 0x00FF
    match firmware_variables::boot::discover_parsed_boot_entries(0x0000, 0x00FF) {
        Ok(found_entries) => {
            println!(
                "Found {} boot entries in range 0x0000-0x00FF:",
                found_entries.len()
            );
            for (entry_id, load_option) in found_entries {
                println!(
                    "Boot{:04X}: {} - Path: {:?} - Attributes: {:?}",
                    entry_id,
                    load_option.description,
                    load_option.file_path_list,
                    load_option.attributes
                );
            }
        }
        Err(e) => {
            eprintln!("Failed to discover boot entries: {}", e);
        }
    }
}
