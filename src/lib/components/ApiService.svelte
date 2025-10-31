<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { BootEntry, ShortcutAction } from "../types";

  export let onstatusfetched: ((portable: boolean) => void) | undefined =
    undefined;
  export let onbootentriesfetched:
    | ((entries: BootEntry[]) => void)
    | undefined = undefined;
  export let onerror: ((error: string) => void) | undefined = undefined;

  let busy = false;

  // Fetch portable status once at app start
  export async function fetchPortableStatus(): Promise<boolean> {
    try {
      const portable = (await invoke("is_portable")) as boolean;
      onstatusfetched?.(portable);
      return portable;
    } catch (e) {
      onstatusfetched?.(false);
      return false;
    }
  }

  // Fetch boot entries from Rust backend
  export async function fetchBootEntries() {
    busy = true;
    try {
      const entries = (await invoke("get_boot_entries")) as BootEntry[];
      onbootentriesfetched?.(entries);
      return entries;
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }

  // Fetch discovered boot entries
  export async function fetchDiscoveredEntries() {
    busy = true;
    try {
      const entries = (await invoke("discover_entries")) as BootEntry[];
      return entries;
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }

  // Set BootNext
  export async function setBootNext(entryId: number) {
    busy = true;
    try {
      await invoke("set_boot_next", { entryId });
      await fetchBootEntries();
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }

  // Unset BootNext
  export async function unsetBootNext() {
    busy = true;
    try {
      await invoke("unset_boot_next");
      await fetchBootEntries();
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }

  // Set boot order
  export async function setBootOrder(newOrder: number[]) {
    busy = true;
    try {
      await invoke("set_boot_order", { order: newOrder });
      await fetchBootEntries();
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }

  // Restart Now
  export async function restartNow() {
    busy = true;
    try {
      await invoke("restart_now");
    } catch (e) {
      onerror?.(String(e));
    } finally {
      busy = false;
    }
  }

  // Create shortcut
  export async function createShortcut(config: {
    name: string;
    action: ShortcutAction;
    entryId?: number;
    reboot: boolean;
    iconId?: string;
  }) {
    busy = true;
    try {
      await invoke("create_shortcut", {
        config: {
          name: config.name,
          action: config.action,
          entry_id: config.entryId,
          reboot: config.reboot,
          icon_id: config.iconId,
        },
      });
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }

  // Set boot to firmware setup
  export async function setBootToFirmwareSetup() {
    busy = true;
    try {
      await invoke("set_boot_fw");
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }

  // Get boot to firmware setup state
  export async function getBootToFirmwareSetupState(): Promise<boolean> {
    try {
      const state = (await invoke("get_boot_fw")) as boolean;
      return state;
    } catch (e) {
      onerror?.(String(e));
      return false;
    }
  }

  // Unset boot to firmware setup
  export async function unsetBootToFirmwareSetup() {
    busy = true;
    try {
      await invoke("unset_boot_fw");
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }
</script>
