<script lang="ts">
  import { onMount } from "svelte";
  import { mockBootEntries } from "./mockBootEntries";
  import type { BootEntry } from "../lib/types";
  import Header from "../lib/components/Header.svelte";
  import BootEntriesList from "../lib/components/BootEntriesList.svelte";
  import ApiService from "../lib/components/ApiService.svelte";
  import ShortcutDialog from "../lib/components/ShortcutDialog.svelte";

  let bootEntries: BootEntry[] = [];
  let originalOrder: number[] = [];
  let error = "";
  let changed = false;
  let busy = false;
  let isPortable: boolean | null = null;
  let apiService: ApiService;

  // Shortcut dialog state
  let showShortcutDialog = false;
  let shortcutEntry: BootEntry | null = null;

  // Move entry up/down
  function moveEntry(idx: number, dir: "up" | "down") {
    if (busy) return;
    const newIdx = dir === "up" ? idx - 1 : idx + 1;
    if (newIdx < 0 || newIdx >= bootEntries.length) return;
    [bootEntries[idx], bootEntries[newIdx]] = [
      bootEntries[newIdx],
      bootEntries[idx],
    ];
    checkForChanges();
  }

  // Check if order has changed
  function checkForChanges() {
    changed =
      JSON.stringify(bootEntries.map((e) => e.id)) !==
      JSON.stringify(originalOrder);
  }

  // Save boot order
  async function saveOrder() {
    await apiService.saveBootOrder(bootEntries.map((e) => e.id));
  }

  // Discard changes
  function discardChanges() {
    if (busy) return;
    bootEntries = originalOrder.map(
      (id) => bootEntries.find((e) => e.id === id)!
    );
    changed = false;
  }

  // Handle events from ApiService (now callback props)
  function handleStatusFetched(portable: boolean) {
    isPortable = portable;
  }

  function handleBootEntriesFetched(entries: BootEntry[]) {
    bootEntries = entries;
    originalOrder = bootEntries.map((e) => e.id);
    changed = false;
  }

  function handleError(errorMessage: string) {
    error = errorMessage;
  }

  // Handle events from BootEntriesList (now callback props)
  function handleEntriesChanged(entries: BootEntry[]) {
    bootEntries = entries;
    checkForChanges();
  }

  function handleMoveUp(index: number) {
    moveEntry(index, "up");
  }

  function handleMoveDown(index: number) {
    moveEntry(index, "down");
  }

  async function handleSetBootNext(entry: BootEntry) {
    await apiService.setBootNext(entry.id);
  }

  async function handleUnsetBootNext() {
    await apiService.unsetBootNext();
  }

  async function handleRestartNow() {
    await apiService.restartNow();
  }

  async function handleMakeDefault(entry: BootEntry) {
    // Move the selected entry to the first position, keeping others in order
    const entryIndex = bootEntries.findIndex((e) => e.id === entry.id);
    if (entryIndex === -1) return;

    // Create new order with selected entry first
    const newBootEntries = [
      bootEntries[entryIndex],
      ...bootEntries.slice(0, entryIndex),
      ...bootEntries.slice(entryIndex + 1),
    ];

    bootEntries = newBootEntries;

    // Save the new boot order
    await apiService.saveBootOrder(bootEntries.map((e) => e.id));
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

    await apiService.createShortcut({
      name: config.name,
      entryId: shortcutEntry.id,
      reboot: config.reboot,
    });
    showShortcutDialog = false;
    shortcutEntry = null;
  }

  function handleShortcutCancel() {
    showShortcutDialog = false;
    shortcutEntry = null;
  }

  if (import.meta.env.DEV) {
    bootEntries = mockBootEntries;
    originalOrder = bootEntries.map((e) => e.id);

    // Mock portable status promise that resolves after 3 seconds
    setTimeout(() => {
      handleStatusFetched(false);
    }, 3000);
  } else {
    onMount(async () => {
      await apiService.fetchPortableStatus();
      await apiService.fetchBootEntries();
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
  <Header {changed} {busy} onsave={saveOrder} ondiscard={discardChanges} />

  {#if error}
    <p class="text-red-600 dark:text-red-400 max-w-2xl mx-auto px-2 mb-4">
      {error}
    </p>
  {/if}

  <BootEntriesList
    {bootEntries}
    {busy}
    {isPortable}
    onentrieschanged={handleEntriesChanged}
    onmoveup={handleMoveUp}
    onmovedown={handleMoveDown}
    onsetbootnext={handleSetBootNext}
    onunsetbootnext={handleUnsetBootNext}
    onrestartnow={handleRestartNow}
    onmakedefault={handleMakeDefault}
    onaddshortcut={handleAddShortcut}
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
