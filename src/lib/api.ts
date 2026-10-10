import { invoke } from "@tauri-apps/api/core";
import { check } from "@tauri-apps/plugin-updater";
import type { BootEntry, ShortcutConfig } from "./types";

export interface FirmwareApi {
  setBootNext(entryId: number): Promise<void>;
  unsetBootNext(): Promise<void>;
  setBootToFirmwareSetup(): Promise<void>;
  unsetBootToFirmwareSetup(): Promise<void>;
}

export class ApiService implements FirmwareApi {
  private activeRequests = 0;

  constructor(private callbacks: {
    onstatusfetched?: (portable: boolean) => void;
    onbootentriesfetched?: (entries: BootEntry[]) => void;
    onerror?: (error: string) => void;
    onbusychange?: (busy: boolean) => void;
  } = {}) {}

  private async request<T>(operation: () => Promise<T>): Promise<T> {
    this.activeRequests++;
    this.callbacks.onbusychange?.(true);
    try {
      return await operation();
    } catch (error) {
      this.callbacks.onerror?.(String(error));
      throw error;
    } finally {
      this.activeRequests--;
      this.callbacks.onbusychange?.(this.activeRequests > 0);
    }
  }

  async fetchPortableStatus(): Promise<boolean> {
    const portable = await this.request(() => invoke<boolean>("is_portable"));
    this.callbacks.onstatusfetched?.(portable);
    return portable;
  }

  getOS(): Promise<string> {
    return this.request(() => invoke<string>("get_os"));
  }

  async fetchBootEntries(): Promise<BootEntry[]> {
    const entries = await this.request(() => invoke<BootEntry[]>("get_boot_entries"));
    this.callbacks.onbootentriesfetched?.(entries);
    return entries;
  }

  fetchDiscoveredEntries(): Promise<BootEntry[]> {
    return this.request(() => invoke<BootEntry[]>("discover_entries"));
  }

  setBootNext(entryId: number): Promise<void> {
    return this.request(() => invoke<void>("set_boot_next", { entryId }));
  }

  unsetBootNext(): Promise<void> {
    return this.request(() => invoke<void>("unset_boot_next"));
  }

  setBootOrder(order: number[]): Promise<void> {
    return this.request(() => invoke<void>("set_boot_order", { order }));
  }

  restartNow(): Promise<void> {
    return this.request(() => invoke<void>("restart_now"));
  }

  createShortcut(config: ShortcutConfig): Promise<void> {
    return this.request(() => invoke<void>("create_shortcut", {
      config: {
        name: config.name,
        action: config.action,
        entry_id: config.entryId,
        reboot: config.reboot,
        icon_id: config.iconId,
      },
    }));
  }

  setBootToFirmwareSetup(): Promise<void> {
    return this.request(() => invoke<void>("set_boot_fw"));
  }

  getBootToFirmwareSetupState(): Promise<boolean> {
    return this.request(() => invoke<boolean>("get_boot_fw"));
  }

  unsetBootToFirmwareSetup(): Promise<void> {
    return this.request(() => invoke<void>("unset_boot_fw"));
  }

  checkForUpdates() {
    return this.request(() => check());
  }
}
