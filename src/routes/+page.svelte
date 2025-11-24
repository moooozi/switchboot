<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import ApiService from "../lib/components/ApiService.svelte";
  import BootEntriesList from "../lib/components/BootEntriesList.svelte";
  import Header from "../lib/components/Header.svelte";
  import ShortcutDialog from "../lib/components/ShortcutDialog.svelte";
  import UpdateDialog from "../lib/components/UpdateDialog.svelte";
  import { getIconId } from "../lib/iconMap";
  import { OrderManager } from "../lib/orderManager";
  import { undoRedoStore } from "../lib/stores/undoRedo";
  import type { BootEntry } from "../lib/types";
  import { ShortcutAction } from "../lib/types";
  import { mockBootEntries } from "./mockBootEntries";

  let bootEntries: BootEntry[] = [];
  let discoveredEntries: BootEntry[] = [];
  let originalOrder: number[] = [];
  let originalEntries: BootEntry[] = [];
  let error = "";
  let changed = false;
  let busy = false;
  let isPortable: boolean | null = null;
  let apiService: ApiService;
  let initialized = false;
  let discoveredEntriesLoading = true;
  let efiSetupState = false;
  let availableUpdate: any = null;
  let showUpdateDialog = false;
  let updateProgress = 0;
  let updateStatus = "";
  let updateTotalSize = 0;

  // Order manager for handling changes with undo/redo
  let orderManager: OrderManager;

  // Shortcut dialog state
  let showShortcutDialog = false;
  let shortcutEntry: BootEntry | null = null;

  $: others = [
    {
      id: -200,
      description: "EFI Firmware Setup",
      is_default: null,
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

  // Initialize order manager when boot entries are fetched
  $: if (bootEntries.length > 0 && !orderManager && apiService) {
    orderManager = new OrderManager(
      bootEntries,
      (entries) => {
        bootEntries = entries;
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

  // Update order manager's discovered entries when they change
  $: if (orderManager && discoveredEntries) {
    orderManager.setDiscoveredEntries(discoveredEntries);
  }

  // Move entry up/down
  function moveEntry(idx: number, dir: "up" | "down") {
    if (busy || !orderManager) return;
    orderManager.moveEntry(idx, dir);
  }
  // Save boot order
  async function saveOrder() {
    try {
      await apiService.setBootOrder(bootEntries.map((e) => e.id));
      originalOrder = bootEntries.map((e) => e.id);
      originalEntries = [...bootEntries];
    } catch (e) {
      // Error is already handled by ApiService's onerror callback
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
    if (!orderManager) return;
    await orderManager.setBootNext(entry.id);
  }

  async function handleUnsetBootNext() {
    if (!orderManager) return;
    await orderManager.unsetBootNext();
  }

  async function handleRestartNow() {
    await apiService.restartNow();
  }

  async function handleMakeDefault(entry: BootEntry) {
    if (!orderManager) return;
    orderManager.makeDefault(entry.id);

    // // Save the new boot order immediately (this action itself is not undoable)
    // await apiService.setBootOrder(bootEntries.map((e) => e.id));
    // originalOrder = bootEntries.map((e) => e.id);
    // originalEntries = [...bootEntries];
  }

  async function handleAddShortcut(entry: BootEntry) {
    shortcutEntry = entry;
    showShortcutDialog = true;
  }

  async function handleShortcutCreate(config: {
    name: string;
    reboot: boolean;
  }) {
    if (!shortcutEntry) return;

    const iconId = getIconId(shortcutEntry.description) || "generic";

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
  }

  async function handleShortcutCancel() {
    showShortcutDialog = false;
    shortcutEntry = null;
  }

  async function handleRebootToFirmwareSetup() {
    if (!orderManager) return;
    await orderManager.setBootToFirmwareSetup();
  }

  async function handleUnsetBootToFirmwareSetup() {
    if (!orderManager) return;
    await orderManager.unsetBootToFirmwareSetup();
  }

  async function handleAddToBootOrder(entry: BootEntry) {
    if (!orderManager) return;
    orderManager.addToBootOrder(entry);
  }

  async function handleRemoveFromBootOrder(entry: BootEntry) {
    if (!orderManager) return;
    orderManager.removeFromBootOrder(entry.id);
  }

  // Undo/Redo functionality
  function handleUndo() {
    undoRedoStore.undo();
  }

  function handleRedo() {
    undoRedoStore.redo();
  }

  async function handleUpdateClick() {
    if (!availableUpdate) return;
    try {
      showUpdateDialog = true;
      updateProgress = 0;
      updateTotalSize = 0;
      updateStatus = "Starting download...";
      busy = true;

      await availableUpdate.downloadAndInstall((event: any) => {
        switch (event.event) {
          case "Started":
            updateTotalSize = event.data.contentLength;
            updateStatus = `Downloading ${(event.data.contentLength / (1024 * 1024)).toFixed(1)} MB...`;
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
      busy = false;
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

  if (false) {
    bootEntries = mockBootEntries;
    originalOrder = bootEntries.map((e) => e.id);

    // Mock portable status promise that resolves after 3 seconds
    setTimeout(() => {
      handleStatusFetched(false);
    }, 3000);
  } else {
    onMount(async () => {
      // Add keyboard shortcuts
      document.addEventListener("keydown", handleKeydown);

      await apiService.fetchPortableStatus();

      // Load boot entries first (fast)
      await apiService.fetchBootEntries();

      // Get EFI Setup state
      efiSetupState = await apiService.getBootToFirmwareSetupState();

      // Check for updates
      try {
        availableUpdate = await apiService.checkForUpdates();
      } catch (e) {
        console.log("Update check failed:", e);
      }

      initialized = true;

      // Load discovered entries asynchronously (slow)
      (async () => {
        try {
          const fetchedEntries = await apiService.fetchDiscoveredEntries();
          // Merge new entries without overwriting existing ones
          const existingIds = new Set(discoveredEntries.map((e) => e.id));
          const newEntries = fetchedEntries.filter(
            (e) => !existingIds.has(e.id)
          );
          discoveredEntries = [...discoveredEntries, ...newEntries];
        } catch (e) {
          onerror?.(String(e));
        } finally {
          discoveredEntriesLoading = false;
        }
      })();
    });

    onDestroy(() => {
      document.removeEventListener("keydown", handleKeydown);
    });
  }
</script>

<ApiService
  bind:this={apiService}
  onstatusfetched={handleStatusFetched}
  onbootentriesfetched={handleBootEntriesFetched}
  onerror={handleError}
/>

<main
  class="bg-neutral-100 dark:bg-neutral-900 text-neutral-900 dark:text-neutral-100 p-5 min-h-svh h-screen flex flex-col font-sans"
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
