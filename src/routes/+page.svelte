<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { dndzone } from "svelte-dnd-action";

  type BootEntry = {
    id: number;
    description: string;
    is_default: boolean;
    is_bootnext: boolean;
  };

  let bootEntries: BootEntry[] = [];
  let originalOrder: number[] = [];
  let loading = true;
  let error = "";
  let changed = false;

  // Fetch boot entries from Rust backend
  async function fetchBootEntries() {
    loading = true;
    error = "";
    try {
      bootEntries = await invoke("get_boot_entries");
      originalOrder = bootEntries.map(e => e.id);
      changed = false;
    } catch (e) {
      error = String(e);
    }
    loading = false;
  }

  // Move entry up/down
  function moveEntry(idx: number, dir: "up" | "down") {
    const newIdx = dir === "up" ? idx - 1 : idx + 1;
    if (newIdx < 0 || newIdx >= bootEntries.length) return;
    [bootEntries[idx], bootEntries[newIdx]] = [bootEntries[newIdx], bootEntries[idx]];
    changed = JSON.stringify(bootEntries.map(e => e.id)) !== JSON.stringify(originalOrder);
  }

  // Set BootNext
  async function setBootNext(entry: BootEntry) {
    await invoke("set_boot_next", { entryId: entry.id });
    await fetchBootEntries();
  }

  // Undo BootNext
  async function unsetBootNext() {
    await invoke("unset_boot_next", { defaultEntry: bootEntries[0].id });
    await fetchBootEntries();
  }

  // Save boot order
  async function saveOrder() {
    await invoke("save_boot_order", { newOrder: bootEntries.map(e => e.id) });
    await fetchBootEntries();
  }

  // Discard changes
  function discardChanges() {
    bootEntries = originalOrder.map(id => bootEntries.find(e => e.id === id)!);
    changed = false;
  }

  // Restart Now
  async function restartNow() {
    await invoke("restart_now");
  }

  function handleDnd(event: CustomEvent<{ items: BootEntry[] }>) {
    const { detail } = event;
    bootEntries = detail.items;
    changed = JSON.stringify(bootEntries.map(e => e.id)) !== JSON.stringify(originalOrder);
  }

  onMount(fetchBootEntries);
</script>

<main class="bg-neutral-100 dark:bg-neutral-900 text-neutral-900 dark:text-neutral-100 p-5  min-h-svh font-sans">
  <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 2rem;">
    <h1 class="text-3xl font-bold tracking-tight">Boot Manager</h1>
    <div style="display: flex; gap: 0.75rem;">
      <button
        class="bg-sky-600 text-white font-semibold hover:bg-sky-700 dark:bg-sky-500 dark:hover:bg-sky-600 transition"
        style="border-radius: 9999px; padding: 0.5rem 1.5rem;"
        on:click={saveOrder}
        disabled={!changed}
      >Save</button>
      <button
        class="bg-neutral-200 text-neutral-800 hover:bg-neutral-300 dark:bg-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-600 transition"
        style="border-radius: 9999px; padding: 0.5rem 1.5rem;"
        on:click={discardChanges}
        disabled={!changed}
      >Discard</button>
    </div>
  </div>
  {#if loading}
    <p class="text-neutral-500 dark:text-neutral-400">Loading...</p>
  {:else if error}
    <p class="text-red-600 dark:text-red-400">{error}</p>
  {:else}
    <div
      use:dndzone={{ items: bootEntries, flipDurationMs: 150, dragDisabled: false }}
      on:consider={handleDnd}
      on:finalize={handleDnd}
      style="display: flex; flex-direction: column; gap: 1rem; margin-bottom: 2.5rem;"
    >
      {#each bootEntries as entry, idx (entry.id)}
        <div
          class="bg-neutral-200 dark:bg-neutral-800 transition-colors"
          style="
            display: flex;
            align-items: center;
            gap: 0.75rem;
            padding: 1rem;
            border-radius: 0.75rem;
            border-width: 1px;
            border-style: solid;
            border-color: 
              {entry.is_default 
                ? 'var(--tw-prose-sky-500, #0ea5e9)' 
                : entry.is_bootnext 
                  ? 'var(--tw-prose-emerald-500, #10b981)' 
                  : 'var(--tw-prose-neutral-200, #e5e7eb)'
              };
            cursor: grab;
          "
          data-id={entry.id}
        >
          <span style="flex: 1;" class="text-base">{entry.description}</span>
          <button
            class="bg-neutral-200 dark:bg-neutral-700 text-neutral-700 dark:text-neutral-200 hover:bg-neutral-300 dark:hover:bg-neutral-600 transition"
            style="border-radius: 9999px; padding: 0.25rem 0.75rem;"
            on:click={() => moveEntry(idx, 'up')}
            disabled={idx === 0}
            aria-label="Move up"
          >↑</button>
          <button
            class="bg-neutral-200 dark:bg-neutral-700 text-neutral-700 dark:text-neutral-200 hover:bg-neutral-300 dark:hover:bg-neutral-600 transition"
            style="border-radius: 9999px; padding: 0.25rem 0.75rem;"
            on:click={() => moveEntry(idx, 'down')}
            disabled={idx === bootEntries.length - 1}
            aria-label="Move down"
          >↓</button>
          {#if entry.is_default}
            <span class="bg-sky-500 text-white text-xs" style="border-radius: 9999px; padding: 0.25rem 0.75rem;">Default</span>
          {:else if entry.is_bootnext}
            <button
              class="bg-emerald-500 text-white text-xs hover:bg-emerald-600 transition"
              style="border-radius: 9999px; padding: 0.25rem 0.75rem;"
              on:click={unsetBootNext}
            >Undo</button>
            <button
              class="bg-amber-500 text-white text-xs hover:bg-amber-600 transition"
              style="border-radius: 9999px; padding: 0.25rem 0.75rem;"
              on:click={restartNow}
            >Restart</button>
          {:else}
            <button
              class="bg-neutral-300 dark:bg-neutral-600 text-neutral-800 dark:text-neutral-200 text-xs hover:bg-sky-500 hover:text-white dark:hover:bg-sky-600 transition"
              style="border-radius: 9999px; padding: 0.25rem 0.75rem;"
              on:click={() => setBootNext(entry)}
            >BootNext</button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</main>

<!-- Add this to your button class or as a global style -->
<style>
  button:disabled,
  button:disabled:hover {
    opacity: 0.5;
    /* Remove background and color changes on hover */
    color: inherit !important;
    box-shadow: none !important;
    filter: none !important;
  }
</style>