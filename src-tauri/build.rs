fn main() {
    // Generate a random PSK and write it to psk.rs
    use rand::Rng;
    use std::fs::File;
    use std::io::Write;

    // Only generate if not already present, or always overwrite for a new key each build
    let mut rng = rand::rng();
    let key: [u8; 32] = rng.random();
    let out_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let psk_path = format!("{}/src/cli/windows/psk.rs", out_dir);
    let mut file = File::create(psk_path).unwrap();
    write!(file, "pub const PSK: [u8; 32] = {:?};", key).unwrap();

    // Call tauri's build as before
    tauri_build::build();
}
