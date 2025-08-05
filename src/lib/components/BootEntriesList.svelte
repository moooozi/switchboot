<script lang="ts">
  import { dndzone } from "svelte-dnd-action";
  import { flip } from "svelte/animate";
  import type { BootEntry } from "../types";
  import BootEntryItem from "./BootEntryItem.svelte";
  import ContextMenuItem from "./ContextMenuItem.svelte";

  export let bootEntries: BootEntry[];
  export let busy: boolean;
  export let isPortable: boolean | null;

  // Callback props instead of events
  export let onentrieschanged: ((entries: BootEntry[]) => void) | undefined =
    undefined;
  export let onmoveup: ((index: number) => void) | undefined = undefined;
  export let onmovedown: ((index: number) => void) | undefined = undefined;
  export let onsetbootnext: ((entry: BootEntry) => void) | undefined =
    undefined;
  export let onunsetbootnext: (() => void) | undefined = undefined;
  export let onrestartnow: (() => void) | undefined = undefined;
  export let onmakedefault: ((entry: BootEntry) => void) | undefined =
    undefined;
  export let onaddshortcut: ((entry: BootEntry) => void) | undefined =
    undefined;

  const flipDuration = 150;

  // Centralized context menu state
  let showContextMenu = false;
  let contextMenuX = 0;
  let contextMenuY = 0;
  let contextMenuEntry: BootEntry | null = null;

  function handleDnd(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;
    const { detail } = event;
    bootEntries = detail.items;
    onentrieschanged?.(bootEntries);
  }

  function handleMoveUp(index: number) {
    onmoveup?.(index);
  }

  function handleMoveDown(index: number) {
    onmovedown?.(index);
  }

  function handleSetBootNext(entry: BootEntry) {
    onsetbootnext?.(entry);
  }

  function handleUnsetBootNext() {
    onunsetbootnext?.();
  }

  function handleRestartNow() {
    onrestartnow?.();
  }

  function handleContextMenu(data: {
    entry: BootEntry;
    mouseEvent: MouseEvent;
  }) {
    const { entry, mouseEvent } = data;
    mouseEvent.preventDefault();

    // Close any existing context menu first
    showContextMenu = false;

    // Calculate position with edge detection
    const menuWidth = 200; // Approximate width of context menu
    const menuHeight = 80; // Approximate height of context menu
    const margin = 10; // Safety margin from edges

    let x = mouseEvent.clientX;
    let y = mouseEvent.clientY;

    // Check right edge
    if (x + menuWidth + margin > window.innerWidth) {
      x = window.innerWidth - menuWidth - margin;
    }

    // Check bottom edge
    if (y + menuHeight + margin > window.innerHeight) {
      y = window.innerHeight - menuHeight - margin;
    }

    // Check left edge (shouldn't happen but just in case)
    if (x < margin) {
      x = margin;
    }

    // Check top edge
    if (y < margin) {
      y = margin;
    }

    contextMenuX = x;
    contextMenuY = y;
    contextMenuEntry = entry;
    showContextMenu = true;
  }

  function closeContextMenu() {
    showContextMenu = false;
    contextMenuEntry = null;
  }

  function handleMakeDefaultFromMenu() {
    if (contextMenuEntry) {
      onmakedefault?.(contextMenuEntry);
      closeContextMenu();
    }
  }

  function handleAddShortcutFromMenu() {
    if (contextMenuEntry) {
      onaddshortcut?.(contextMenuEntry);
      closeContextMenu();
    }
  }

  // Close context menu when clicking elsewhere
  function handleDocumentClick() {
    closeContextMenu();
  }
</script>

<svelte:document on:click={handleDocumentClick} />

<div
  class="flex-1 overflow-y-auto flex flex-col gap-4 mb-2 bg-neutral-100 dark:bg-neutral-900 px-2 max-w-2xl w-full mx-auto"
  use:dndzone={{
    items: bootEntries,
    flipDurationMs: flipDuration,
    dropTargetStyle: {},

    dragDisabled: busy,
  }}
  on:consider={handleDnd}
  on:finalize={handleDnd}
>
  {#each bootEntries as entry, idx (entry.id)}
    <div animate:flip={{ duration: flipDuration }}>
      <BootEntryItem
        {entry}
        index={idx}
        totalEntries={bootEntries.length}
        {busy}
        onmoveup={handleMoveUp}
        onmovedown={handleMoveDown}
        onsetbootnext={handleSetBootNext}
        onunsetbootnext={handleUnsetBootNext}
        onrestartnow={handleRestartNow}
        oncontextmenu={handleContextMenu}
      />
    </div>
  {/each}
</div>

<!-- Centralized Context Menu -->
{#if showContextMenu && contextMenuEntry}
  <div
    class="fixed w-max flex flex-col bg-white dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-600 rounded-lg shadow-lg py-2 z-50"
    style="left: {contextMenuX}px; top: {contextMenuY}px;"
  >
    <ContextMenuItem
      disabled={contextMenuEntry.is_default || busy}
      onclick={handleMakeDefaultFromMenu}
    >
      Make Default
    </ContextMenuItem>
    <ContextMenuItem
      disabled={busy || isPortable !== false}
      title={isPortable === true
        ? "Shortcuts are not available in portable mode"
        : isPortable === null
          ? "Loading portable mode status..."
          : ""}
      onclick={handleAddShortcutFromMenu}
    >
      Add Shortcut
    </ContextMenuItem>
  </div>
{/if}

<style>
  /* Remove white border/background from dragged element */
  :global(#dnd-action-dragged-el) {
    outline: none !important;
  }
</style>
