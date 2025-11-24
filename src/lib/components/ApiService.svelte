<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { check } from "@tauri-apps/plugin-updater";
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

  // Check for updates
  export async function checkForUpdates() {
    busy = true;
    try {
      const update = await check();
      return update;
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }

  // Mock update check for testing UI
  export async function checkForMockUpdates() {
    busy = true;
    try {
      // Simulate network delay
      await new Promise((resolve) => setTimeout(resolve, 1000));

      // Return a mock update object
      return {
        version: "0.3.0",
        body: "This is a mock update for testing the UI.\n\n- Added new features\n- Fixed some bugs\n- Improved performance",
        downloadAndInstall: async (callback: (event: any) => void) => {
          const totalSize = 15 * 1024 * 1024; // 15 MB
          let downloaded = 0;

          // Simulate download progress
          callback({ event: "Started", data: { contentLength: totalSize } });

          const interval = setInterval(() => {
            const chunkSize = Math.random() * 500000 + 100000; // Random chunk between 100KB-600KB
            downloaded += chunkSize;

            if (downloaded >= totalSize) {
              downloaded = totalSize;
              callback({ event: "Progress", data: { chunkLength: chunkSize } });
              callback({ event: "Finished" });
              clearInterval(interval);

              // Simulate installation delay
              setTimeout(() => {
                // Mock successful installation
              }, 2000);
            } else {
              callback({ event: "Progress", data: { chunkLength: chunkSize } });
            }
          }, 200); // Update every 200ms
        },
      };
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }
</script>
