fn main() {
    
    // Generate PSK for Windows if needed
    #[cfg(target_os = "windows")]
    if let Err(e) = generate_psk() {
        eprintln!("Failed to generate PSK: {}", e);
        std::process::exit(1);
    }

    // Call tauri's build as before
    tauri_build::build();
}


#[cfg(target_os = "windows")]
fn generate_psk() -> Result<(), String> {
        // Generate a random PSK and write it to psk.rs
    use rand::Rng;
    use std::fs::File;
    use std::io::Write;

    let mut rng = rand::rng();
    let key: [u8; 32] = rng.random();
    let out_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let psk_path = format!("{}/src/cli/windows/psk.rs", out_dir);
    let mut file = File::create(psk_path).unwrap();
    write!(file, "pub const PSK: [u8; 32] = {:?};", key).unwrap();
    Ok(())
}
