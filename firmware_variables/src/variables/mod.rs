use bitflags::bitflags;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(unix)]
pub mod unix;
#[cfg(unix)]
pub use unix::*;

bitflags! {
    pub struct Attributes: u32 {
        const NON_VOLATILE = 0x00000001;
        const BOOT_SERVICE_ACCESS = 0x00000002;
        const RUNTIME_ACCESS = 0x00000004;
        const HARDWARE_ERROR_RECORD = 0x00000008;
        const AUTHENTICATED_WRITE_ACCESS = 0x00000010;
        const TIME_BASED_AUTHENTICATED_WRITE_ACCESS = 0x00000020;
        const APPEND_WRITE = 0x00000040;
    }
}

pub const DEFAULT_ATTRIBUTES: Attributes = Attributes::NON_VOLATILE
    .union(Attributes::BOOT_SERVICE_ACCESS)
    .union(Attributes::RUNTIME_ACCESS);

pub const GLOBAL_NAMESPACE: &str = "{8BE4DF61-93CA-11d2-AA0D-00E098032B8C}";
