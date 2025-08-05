use std::sync::OnceLock;

/// Global application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    #[cfg(target_os = "windows")]
    pub portable_mode: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            #[cfg(target_os = "windows")]
            portable_mode: false,
        }
    }
}

static CONFIG: OnceLock<AppConfig> = OnceLock::new();
static DEFAULT_CONFIG: AppConfig = AppConfig {
    #[cfg(target_os = "windows")]
    portable_mode: false,
};

/// Initialize the global configuration
pub fn init_config(config: AppConfig) {
    CONFIG.set(config).ok();
}

/// Get the global configuration
pub fn get_config() -> &'static AppConfig {
    CONFIG.get().unwrap_or(&DEFAULT_CONFIG)
}

/// Check if portable mode is enabled (Windows only)
#[cfg(target_os = "windows")]
pub fn is_portable_mode() -> bool {
    get_config().portable_mode
}

#[cfg(not(target_os = "windows"))]
pub fn is_portable_mode() -> bool {
    false
}
