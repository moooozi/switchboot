use std::env;

fn main() {
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
