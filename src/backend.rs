use switchboot_core as core;
use switchboot_ui::{Backend, BootTarget, Entry, Shortcut, Snapshot};

/// Firmware uses the privilege-separated CLI transport. Updates use the
/// unelevated host adapter; they never execute through firmware IPC.
#[derive(Default)]
pub struct FirmwareBackend {
    #[cfg(windows)]
    updates: crate::updater::Updates,
}

fn entries(values: Vec<core::BootEntry>) -> Vec<Entry> {
    values
        .into_iter()
        .map(|entry| Entry {
            id: entry.id,
            description: entry.description,
        })
        .collect()
}

impl Backend for FirmwareBackend {
    #[cfg(windows)]
    fn upgrade_shutdown_requested(&self) -> Result<bool, String> {
        self.updates.shutdown_requested()
    }

    fn updates_supported(&self) -> bool {
        cfg!(windows) && !core::is_portable()
    }

    #[cfg(windows)]
    fn check_update(&self) -> Result<Option<switchboot_ui::UpdateRelease>, String> {
        crate::updater::check()
    }

    #[cfg(windows)]
    fn install_update_with_progress(
        &self,
        version: String,
        progress: std::sync::Arc<dyn Fn(switchboot_ui::UpdateProgress) + Send + Sync>,
    ) -> Result<(), String> {
        self.updates.install(&version, move |downloaded, total| {
            progress(switchboot_ui::UpdateProgress { downloaded, total });
        })
    }

    #[cfg(windows)]
    fn poll_update(&self) -> Result<bool, String> {
        self.updates.poll()
    }

    fn load(&self) -> Result<Snapshot, String> {
        Ok(Snapshot {
            entries: entries(core::get_boot_entries()?),
            order: core::get_boot_order()?,
            boot_next: core::get_boot_next()?,
            current: core::get_boot_current()?,
            firmware_setup: core::get_boot_fw()?,
            portable: core::is_portable(),
        })
    }

    fn discover(&self) -> Result<Vec<Entry>, String> {
        core::discover_entries().map(entries)
    }

    fn set_order(&self, order: Vec<u16>) -> Result<(), String> {
        core::set_boot_order(order)
    }

    fn set_next(&self, next: Option<u16>) -> Result<(), String> {
        match next {
            Some(id) => core::set_boot_next(id),
            None => core::unset_boot_next(),
        }
    }

    fn set_firmware(&self, enabled: bool) -> Result<(), String> {
        if enabled {
            core::set_boot_fw()
        } else {
            core::unset_boot_fw()
        }
    }

    fn create_shortcut(&self, shortcut: Shortcut) -> Result<(), String> {
        let (action, entry_id) = match shortcut.target {
            BootTarget::Entry(id) => (core::ShortcutAction::SetBootNext, Some(id)),
            BootTarget::FirmwareSetup => (core::ShortcutAction::SetFirmwareSetup, None),
        };
        core::create_shortcut(core::ShortcutConfig {
            name: shortcut.name,
            action,
            entry_id,
            reboot: shortcut.reboot,
            icon_id: shortcut.icon_id,
        })
    }

    fn restart(&self) -> Result<(), String> {
        core::restart_now()
    }
}
