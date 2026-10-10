<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { ApiService } from "../lib/api";
  import type { Update } from "@tauri-apps/plugin-updater";
  import BootEntriesList from "../lib/components/BootEntriesList.svelte";
  import Header from "../lib/components/Header.svelte";
  import ShortcutDialog from "../lib/components/ShortcutDialog.svelte";
  import UpdateDialog from "../lib/components/UpdateDialog.svelte";
  import { getIconId } from "../lib/iconMap";
  import { OrderManager } from "../lib/orderManager";
  import { undoRedoStore } from "../lib/stores/undoRedo";
  import type { BootEntry } from "../lib/types";
  import { ShortcutAction } from "../lib/types";


  let bootEntries: BootEntry[] = [];
  let discoveredEntries: BootEntry[] = [];
  let originalOrder: number[] = [];
  let originalEntries: BootEntry[] = [];
  let error = "";
  let changed = false;
  let busy = true;
  let apiBusy = false;
  let updateBusy = false;
  $: busy = apiBusy || updateBusy || $undoRedoStore.busy || !initialized;
  let isPortable: boolean | null = null;
  let os: string = "unknown";
  const apiService = new ApiService({
    onstatusfetched: handleStatusFetched,
    onbootentriesfetched: handleBootEntriesFetched,
    onerror: handleError,
    onbusychange: (value) => { apiBusy = value; },
  });
  let initialized = false;
  let discoveredEntriesLoading = true;
  let efiSetupState = false;
  let availableUpdate: Update | null = null;
  let showUpdateDialog = false;
  let updateProgress = 0;
  let updateStatus = "";
  let updateTotalSize = 0;

  // Order manager for handling changes with undo/redo
  let orderManager: OrderManager;

  // Shortcut dialog state
  let showShortcutDialog = false;
  let shortcutEntry: BootEntry | null = null;

  // Merge boot next state from discoveredEntries into bootEntries
  $: if (discoveredEntries.length > 0 && bootEntries.length > 0) {
    bootEntries = bootEntries.map((entry) => {
      const discovered = discoveredEntries.find((d) => d.id === entry.id);
      return discovered
        ? { ...entry, is_bootnext: discovered.is_bootnext }
        : entry;
    });
  }

  $: others = [
    {
      id: -200,
      description: "EFI Firmware Setup",
      is_bootnext: efiSetupState,
      is_current: false,
    },
    ...discoveredEntries.filter(
      (e) => !bootEntries.some((b) => b.id === e.id) && e.id !== -200
    ),
  ];
  $: if (initialized)
    console.log(
      `Current Boot order: [${bootEntries.map((e) => e.id).join(",")}]`
    );
  $: changed =
    initialized &&
    JSON.stringify(bootEntries.map((e) => e.id)) !==
      JSON.stringify(originalOrder);

  // Derive default entry ID from original order
  $: defaultEntryId = originalOrder[0] ?? null;


  // Update order manager's discovered entries when they change
  $: if (orderManager && discoveredEntries) {
    orderManager.setDiscoveredEntries(discoveredEntries);
  }
  $: if (orderManager) orderManager.setFirmwareSetupState(efiSetupState);

  // Move entry up/down
  function moveEntry(idx: number, dir: "up" | "down") {
    if (busy || !orderManager) return;
    orderManager.moveEntry(idx, dir);
  }
  // Save boot order
  async function saveOrder() {
    if (busy || !changed) return;
    const submittedEntries = bootEntries.map((entry) => ({ ...entry }));
    const submittedOrder = submittedEntries.map((entry) => entry.id);
    await runUiOperation(() => undoRedoStore.runExclusive(async () => {
      await apiService.setBootOrder(submittedOrder);
      originalOrder = submittedOrder;
      originalEntries = submittedEntries;
    }));
  }

  async function runUiOperation(operation: () => Promise<unknown>) {
    try {
      await operation();
    } catch (error) {
      handleError(String(error));
    }
  }

  // Discard changes
  function discardChanges() {
    if (busy || !orderManager) return;
    orderManager.discardToOriginal(originalEntries);
  }

  // Handle events from ApiService (now callback props)
  function handleStatusFetched(portable: boolean) {
    isPortable = portable;
  }

  function handleBootEntriesFetched(entries: BootEntry[]) {
    bootEntries = entries;
    originalEntries = [...entries];
    originalOrder = bootEntries.map((e) => e.id);

    // Initialize discovered entries with boot entries
    discoveredEntries = [...entries];

    undoRedoStore.clear();
    // Initialize order manager
    orderManager = new OrderManager(
      entries,
      (updatedEntries) => {
        bootEntries = updatedEntries;
      },
      (updatedDiscovered) => {
        discoveredEntries = updatedDiscovered;
      },
      (newEfiState) => {
        efiSetupState = newEfiState;
      },
      apiService
    );
  }

  function handleError(errorMessage: string) {
    error = errorMessage;
  }

  // Handle events from BootEntriesList (now callback props)
  function handleEntriesChanged(entries: BootEntry[]) {
    // Just update local state for UI during dragging - actual change recording happens in drag end
    bootEntries = entries;
  }

  function handleDragStart() {
    orderManager?.startDrag();
  }

  function handleDragEnd(entries: BootEntry[]) {
    orderManager?.endDrag(entries);
  }

  function handleMoveUp(index: number) {
    moveEntry(index, "up");
  }

  function handleMoveDown(index: number) {
    moveEntry(index, "down");
  }

  async function handleSetBootNext(entry: BootEntry) {
    if (busy || !orderManager) return;
    await runUiOperation(() => orderManager.setBootNext(entry.id));
  }

  async function handleUnsetBootNext() {
    if (busy || !orderManager) return;
    await runUiOperation(() => orderManager.unsetBootNext());
  }

  async function handleRestartNow() {
    if (busy) return;
    await runUiOperation(() => apiService.restartNow());
  }

  async function handleMakeDefault(entry: BootEntry) {
    if (busy || !orderManager) return;
    orderManager.makeDefault(entry.id);
  }

  async function handleAddShortcut(entry: BootEntry) {
    shortcutEntry = entry;
    showShortcutDialog = true;
  }

  async function handleShortcutCreate(config: {
    name: string;
    reboot: boolean;
  }) {
    if (busy || !shortcutEntry) return;

    const iconId = getIconId(shortcutEntry.description) || "generic";

    try {
      await apiService.createShortcut({
        name: config.name,
        action:
          shortcutEntry.id === -200
            ? ShortcutAction.SetFirmwareSetup
            : ShortcutAction.SetBootNext,
        entryId: shortcutEntry.id === -200 ? undefined : shortcutEntry.id,
        reboot: config.reboot,
        iconId,
      });
      showShortcutDialog = false;
      shortcutEntry = null;
    } catch (error) {
      handleError(String(error));
    }
  }

  async function handleShortcutCancel() {
    showShortcutDialog = false;
    shortcutEntry = null;
  }

  async function handleRebootToFirmwareSetup() {
    if (busy || !orderManager) return;
    await runUiOperation(() => orderManager.setBootToFirmwareSetup());
  }

  async function handleUnsetBootToFirmwareSetup() {
    if (busy || !orderManager) return;
    await runUiOperation(() => orderManager.unsetBootToFirmwareSetup());
  }

  async function handleAddToBootOrder(entry: BootEntry, position?: number) {
    if (busy || !orderManager) return;
    orderManager.addToBootOrder(entry, position);
  }

  async function handleRemoveFromBootOrder(entry: BootEntry) {
    if (busy || !orderManager) return;
    orderManager.removeFromBootOrder(entry.id);
  }

  // Undo/Redo functionality
  async function handleUndo() {
    if (busy) return;
    await runUiOperation(() => undoRedoStore.undo());
  }

  async function handleRedo() {
    if (busy) return;
    await runUiOperation(() => undoRedoStore.redo());
  }

  async function handleUpdateClick() {
    if (busy || !availableUpdate) return;
    try {
      showUpdateDialog = true;
      updateProgress = 0;
      updateTotalSize = 0;
      updateStatus = "Starting download...";
      updateBusy = true;

      await availableUpdate.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            updateTotalSize = event.data.contentLength ?? 0;
            updateStatus = `Downloading ${(updateTotalSize / (1024 * 1024)).toFixed(1)} MB...`;
            break;
          case "Progress":
            updateProgress += event.data.chunkLength;
            const percentage =
              updateTotalSize > 0
                ? (updateProgress / updateTotalSize) * 100
                : 0;
            updateStatus = `Downloaded ${(updateProgress / (1024 * 1024)).toFixed(1)} MB of ${(updateTotalSize / (1024 * 1024)).toFixed(1)} MB (${percentage.toFixed(1)}%)`;
            break;
          case "Finished":
            updateStatus = "Download finished, installing...";
            updateProgress = updateTotalSize;
            break;
        }
      });

      updateStatus = "Update installed successfully!";
      // Clear the available update since it's now installed
      availableUpdate = null;

      // Close dialog after a short delay
      setTimeout(() => {
        showUpdateDialog = false;
      }, 2000);
    } catch (e) {
      console.error("Update installation failed:", e);
      updateStatus = "Update failed";
      setTimeout(() => {
        showUpdateDialog = false;
      }, 2000);
    } finally {
      updateBusy = false;
    }
  }

  // Keyboard shortcuts
  function handleKeydown(event: KeyboardEvent) {
    if (event.ctrlKey || event.metaKey) {
      if (event.key === "z" && !event.shiftKey) {
        event.preventDefault();
        handleUndo();
      } else if (event.key === "y" || (event.key === "z" && event.shiftKey)) {
        event.preventDefault();
        handleRedo();
      } else if (event.key === "s") {
        event.preventDefault();
        saveOrder();
      }
    }
  }

  onMount(async () => {
    document.addEventListener("keydown", handleKeydown);
    try {
      await apiService.fetchPortableStatus();
      os = await apiService.getOS();
      await apiService.fetchBootEntries();
      efiSetupState = await apiService.getBootToFirmwareSetupState();
      orderManager.setFirmwareSetupState(efiSetupState);

      if (!isPortable && os !== "linux") {
        try {
          availableUpdate = await apiService.checkForUpdates();
        } catch (error) {
          console.log("Update check failed:", error);
        }
      }
      initialized = true;

      const fetchedEntries = await apiService.fetchDiscoveredEntries();
      const existingIds = new Set(discoveredEntries.map((entry) => entry.id));
      discoveredEntries = [
        ...discoveredEntries,
        ...fetchedEntries.filter((entry) => !existingIds.has(entry.id)),
      ];
    } catch (error) {
      handleError(String(error));
    } finally {
      discoveredEntriesLoading = false;
    }
  });

  onDestroy(() => {
    document.removeEventListener("keydown", handleKeydown);
  });
</script>

<!-- Reduced right padding (pr-3 = 12px) to match scrollbar width and prevent layout shift -->
<main
  class="bg-neutral-100 dark:bg-neutral-900 text-neutral-900 dark:text-neutral-100 px-5 py-5 pr-3 min-h-svh h-screen flex flex-col font-sans"
  on:contextmenu|preventDefault
>
  <Header
    {changed}
    {busy}
    onsave={saveOrder}
    ondiscard={discardChanges}
    onundo={handleUndo}
    onredo={handleRedo}
    onupdateclick={handleUpdateClick}
    {availableUpdate}
  />

  {#if error}
    <p class="text-red-600 dark:text-red-400 max-w-2xl mx-auto px-2 mb-4">
      {error}
    </p>
  {/if}

  <BootEntriesList
    {bootEntries}
    {busy}
    {isPortable}
    {others}
    {discoveredEntriesLoading}
    {defaultEntryId}
    onentrieschanged={handleEntriesChanged}
    ondragstart={handleDragStart}
    ondragend={handleDragEnd}
    onmoveup={handleMoveUp}
    onmovedown={handleMoveDown}
    onsetbootnext={handleSetBootNext}
    onunsetbootnext={handleUnsetBootNext}
    onsetboottofirmwaresetup={handleRebootToFirmwareSetup}
    onunsetboottofirmwaresetup={handleUnsetBootToFirmwareSetup}
    onrestartnow={handleRestartNow}
    onmakedefault={handleMakeDefault}
    onaddshortcut={handleAddShortcut}
    onaddtobootorder={handleAddToBootOrder}
    onremovefrombootorder={handleRemoveFromBootOrder}
  />
</main>

<!-- Shortcut Dialog -->
{#if shortcutEntry}
  <ShortcutDialog
    entry={shortcutEntry}
    visible={showShortcutDialog}
    oncreate={handleShortcutCreate}
    oncancel={handleShortcutCancel}
  />
{/if}

<!-- Update Progress Dialog -->
<UpdateDialog
  visible={showUpdateDialog}
  status={updateStatus}
  progress={updateProgress}
  totalSize={updateTotalSize}
/>

<style>
  :global(::-webkit-scrollbar) {
    width: 12px;
  }
  :global(::-webkit-scrollbar-thumb) {
    background: #444;
    border-radius: 6px;
  }
  :global(.dark ::-webkit-scrollbar-thumb) {
    background: #222;
  }
</style>
