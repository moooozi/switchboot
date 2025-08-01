
#[cfg(target_os = "windows")]
mod daemon;
#[cfg(target_os = "linux")]
mod single_run;

#[cfg(target_os = "linux")]
pub use single_run::call_cli;

#[cfg(target_os = "windows")]
pub use daemon::run_daemon;
