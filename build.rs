use std::{env, fs, path::Path};

fn main() {
    validate_version().expect("invalid build version");
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
