use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let package = manifest.join("../../Cargo.toml");
    println!("cargo:rerun-if-changed={}", package.display());
    let metadata: toml::Value =
        toml::from_str(&fs::read_to_string(package).expect("read root Cargo.toml"))
            .expect("parse root Cargo.toml");
    let version = metadata["package"]["version"]
        .as_str()
        .expect("application version");
    let identifier = "dev.zidane.switchboot";
    let source = format!(
        "pub const APP_IDENTIFIER: &str = {identifier:?};\n\
         pub const APP_NAME: &str = \"Switchboot\";\n\
         pub const APP_VERSION: &str = {version:?};\n\
         pub const APP_IDENTIFIER_VERSION: &str = {pipe:?};\n",
        pipe = format!("{identifier}-{version}"),
    );
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("app_build_info.rs"),
        source,
    )
    .expect("write build metadata");
}
