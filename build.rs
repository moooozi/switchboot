use std::{env, fs, path::Path};

fn main() {
    validate_version().expect("invalid build version");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let icon = "crates/switchboot_core/icons/icon.ico";
        println!("cargo:rerun-if-changed={icon}");
        winresource::WindowsResource::new()
            .set_icon(icon)
            .set("ProductName", "Switchboot")
            .set("OriginalFilename", "switchboot.exe")
            .compile()
            .expect("embed Windows application icon and version resources");
    }
}

/// Release preparation owns version changes; builds only validate them.
fn validate_version() -> Result<(), String> {
    let manifest = env::var("CARGO_MANIFEST_DIR").map_err(|e| e.to_string())?;
    let package = Path::new(&manifest).join("package.json");
    println!("cargo:rerun-if-changed={}", package.display());
    let metadata: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(package).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let version = metadata["version"]
        .as_str()
        .ok_or("missing package.json version")?;
    let cargo_version = env::var("CARGO_PKG_VERSION").map_err(|e| e.to_string())?;
    if version != cargo_version {
        return Err(format!(
            "package.json version {version} differs from Cargo package version {cargo_version}; synchronize both before building"
        ));
    }
    Ok(())
}
