<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { dndzone } from "svelte-dnd-action";

  type BootEntry = {
    id: number;
    description: string;
    is_default: boolean;
    is_bootnext: boolean;
    is_current: boolean;
  };

  let bootEntries: BootEntry[] = [];
  let originalOrder: number[] = [];
  let error = "";
  let changed = false;
  let busy = false;

  // Fetch boot entries from Rust backend
  async function fetchBootEntries() {
    busy = true;
    error = "";
    try {
      bootEntries = await invoke("get_boot_entries");
      originalOrder = bootEntries.map(e => e.id);
      changed = false;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // Move entry up/down
  function moveEntry(idx: number, dir: "up" | "down") {
    if (busy) return;
    const newIdx = dir === "up" ? idx - 1 : idx + 1;
    if (newIdx < 0 || newIdx >= bootEntries.length) return;
    [bootEntries[idx], bootEntries[newIdx]] = [bootEntries[newIdx], bootEntries[idx]];
    changed = JSON.stringify(bootEntries.map(e => e.id)) !== JSON.stringify(originalOrder);
  }

  // Set BootNext
  async function setBootNext(entry: BootEntry) {
    busy = true;
    try {
      await invoke("set_boot_next", { entryId: entry.id });
      await fetchBootEntries();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // Undo BootNext
  async function unsetBootNext() {
    busy = true;
    try {
      await invoke("unset_boot_next");
      await fetchBootEntries();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // Save boot order
  async function saveOrder() {
    busy = true;
    try {
      await invoke("save_boot_order", { newOrder: bootEntries.map(e => e.id) });
      await fetchBootEntries();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // Discard changes
  function discardChanges() {
    if (busy) return;
    bootEntries = originalOrder.map(id => bootEntries.find(e => e.id === id)!);
    changed = false;
  }

  // Restart Now
  async function restartNow() {
    busy = true;
    try {
      await invoke("restart_now");
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function handleDnd(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;
    const { detail } = event;
    bootEntries = detail.items;
    changed = JSON.stringify(bootEntries.map(e => e.id)) !== JSON.stringify(originalOrder);
  }

  onMount(fetchBootEntries);
</script>

<main class="bg-neutral-100 dark:bg-neutral-900 text-neutral-900 dark:text-neutral-100 p-5 min-h-svh h-screen flex flex-col font-sans" on:contextmenu|preventDefault>
  <div class="container max-w-2xl mx-auto flex items-center justify-between mb-8 px-3">
    <h1 class="text-3xl font-bold tracking-tight select-none">Switchboot</h1>
    <div class="flex gap-3">
      <button
        class="bg-sky-600 text-white font-semibold hover:bg-sky-700 dark:bg-sky-500 dark:hover:bg-sky-600 transition rounded-full px-6 py-2"
        on:click={saveOrder}
        disabled={!changed || busy}
        title="Save the current boot order"
      >Save</button>
      <button
        class="bg-neutral-200 text-neutral-800 hover:bg-neutral-300 dark:bg-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-600 transition rounded-full px-6 py-2"
        on:click={discardChanges}
        disabled={!changed || busy}
        title="Discard boot order changes"
      >Discard</button>
    </div>
  </div>
  {#if error}
    <p class="text-red-600 dark:text-red-400">{error}</p>
  {/if}
  <div
    class="flex-1 overflow-y-auto flex flex-col gap-4 mb-2 bg-neutral-100 dark:bg-neutral-900 px-2 max-w-2xl w-full mx-auto"
    use:dndzone={{ items: bootEntries, flipDurationMs: 150, dragDisabled: busy }}
    on:consider={handleDnd}
    on:finalize={handleDnd}
  >
    {#each bootEntries as entry, idx (entry.id)}
      <div
        class={`flex items-center gap-3 p-4 rounded-xl border transition-colors cursor-grab
          ${entry.is_default
            ? "border-sky-500"
            : entry.is_bootnext
              ? "border-emerald-500"
              : "border-neutral-200 dark:border-neutral-700"}
          bg-neutral-200 dark:bg-neutral-800`}
        data-id={entry.id}
      >
        <span class="flex-1 text-base">{entry.description}</span>
        {#if entry.is_current}
          <span
            class="bg-purple-300 text-purple-800 dark:bg-purple-600 dark:text-white text-xs font-semibold px-2 py-1 rounded-full mr-2 select-none "
            title="This entry was used to boot the current OS"
          >
            Current
          </span>
        {/if}
        {#if entry.is_default}
          <button
            disabled={true}
            class="bg-sky-700 text-white text-xs dark:bg-sky-600 transition rounded-full px-3 py-1"
            title="This is the default firmware boot entry"
          >Default</button>
        {:else if entry.is_bootnext}
          <button
            class="bg-emerald-500 text-white text-xs hover:bg-emerald-600 transition rounded-full px-3 py-1"
            on:click={unsetBootNext}
            title="Unset BootNext (cancel one-time boot override)"
            disabled={busy}
          >Undo</button>
          <button
            class="bg-amber-500 text-white text-xs hover:bg-amber-600 transition rounded-full px-3 py-1"
            on:click={restartNow}
            title="Restart now to boot this entry next"
            disabled={busy}
          >Restart</button>
        {:else}
          <button
            class="bg-neutral-300 dark:bg-neutral-600 text-neutral-800 dark:text-neutral-200 text-xs hover:bg-sky-500 hover:text-white dark:hover:bg-sky-600 transition rounded-full px-3 py-1"
            on:click={() => setBootNext(entry)}
            title="Set this entry as BootNext (one-time boot override)"
            disabled={busy}
          >BootNext</button>
        {/if}
        <button
          class="bg-neutral-300 dark:bg-neutral-700 text-neutral-700 dark:text-neutral-200 hover:brightness-95 dark:hover:bg-neutral-600 transition rounded-full px-3 py-1"
          on:click={() => moveEntry(idx, 'up')}
          disabled={idx === 0 || busy}
          aria-label="Move up"
          title="Move entry up"
        >↑</button>
        <button
          class="bg-neutral-300 dark:bg-neutral-700 text-neutral-700 dark:text-neutral-200 hover:brightness-95 dark:hover:bg-neutral-600 transition rounded-full px-3 py-1"
          on:click={() => moveEntry(idx, 'down')}
          disabled={idx === bootEntries.length - 1 || busy}
          aria-label="Move down"
          title="Move entry down"
        >↓</button>
      </div>
    {/each}
  </div>
</main>

<style>
  button:disabled,
  button:disabled:hover {
    opacity: 0.5;
    color: inherit !important;
    box-shadow: none !important;
    filter: none !important;
  }

  ::-webkit-scrollbar {
    width: 12px;
  }
  ::-webkit-scrollbar-thumb {
    background: #444;
    border-radius: 6px;
  }
  .dark ::-webkit-scrollbar-thumb {
    background: #222;
  }
</style>