// Shared constants for switchboot

pub const PIPE_SERVER_WAIT_TIMEOUT: u64 = 5; // 5 seconds
pub const SERVICE_NAME: &str = "swboot-cli";
pub const SERVICE_DISPLAY_NAME: &str = "Switchboot System Service";
pub const SERVICE_START_TIMEOUT: u64 = 10; // seconds - increased to allow for service initialization
