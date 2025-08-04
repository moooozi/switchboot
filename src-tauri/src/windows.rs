#[cfg(target_os = "windows")]
use std::sync::OnceLock;

#[cfg(target_os = "windows")]
static IS_PORTABLE: OnceLock<bool> = OnceLock::new();

#[cfg(target_os = "windows")]
pub fn is_portable_mode() -> bool {
    *IS_PORTABLE.get_or_init(|| std::env::args().any(|arg| arg == "--portable"))
}
