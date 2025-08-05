<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { BootEntry } from "../types";

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

  // Set BootNext
  export async function setBootNext(entryId: number) {
    busy = true;
    try {
      await invoke("set_boot_next", { entryId });
      await fetchBootEntries();
    } catch (e) {
      onerror?.(String(e));
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
    } finally {
      busy = false;
    }
  }

  // Save boot order
  export async function saveBootOrder(newOrder: number[]) {
    busy = true;
    try {
      await invoke("save_boot_order", { newOrder });
      await fetchBootEntries();
    } catch (e) {
      onerror?.(String(e));
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
    entryId: number;
    reboot: boolean;
  }) {
    busy = true;
    try {
      await invoke("create_shortcut", {
        config: {
          name: config.name,
          entry_id: config.entryId,
          reboot: config.reboot,
        },
      });
    } catch (e) {
      onerror?.(String(e));
      throw e;
    } finally {
      busy = false;
    }
  }
</script>
