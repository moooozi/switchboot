<script lang="ts">
  import { dndzone } from "svelte-dnd-action";
  import { flip } from "svelte/animate";
  import type { BootEntry } from "../types";
  import BootEntryItem from "./BootEntryItem.svelte";
  import { openContextMenu } from "../stores/contextMenu";

  export let bootEntries: BootEntry[];
  export let busy: boolean;
  export let isPortable: boolean | null;
  export let others: BootEntry[] = [];
  export let discoveredEntriesLoading = false;

  // Callback props instead of events
  export let onentrieschanged: ((entries: BootEntry[]) => void) | undefined =
    undefined;
  export let ondragstart: (() => void) | undefined = undefined;
  export let ondragend: ((entries: BootEntry[]) => void) | undefined = undefined;
  export let onmoveup: ((index: number) => void) | undefined = undefined;
  export let onmovedown: ((index: number) => void) | undefined = undefined;
  export let onsetbootnext: ((entry: BootEntry) => void) | undefined =
    undefined;
  export let onunsetbootnext: (() => void) | undefined = undefined;
  export let onsetboottofirmwaresetup: (() => void) | undefined = undefined;
  export let onunsetboottofirmwaresetup: (() => void) | undefined = undefined;
  export let onrestartnow: (() => void) | undefined = undefined;
  export let onmakedefault: ((entry: BootEntry) => void) | undefined =
    undefined;
  export let onaddshortcut: ((entry: BootEntry) => void) | undefined =
    undefined;
  export let onaddtobootorder: ((entry: BootEntry) => void) | undefined =
    undefined;
  export let onremovefrombootorder: ((entry: BootEntry) => void) | undefined =
    undefined;

  const flipDuration = 150;

  function handleDndConsider(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;
    // Capture the starting order when drag begins
    ondragstart?.();
    const { detail } = event;
    bootEntries = detail.items;
    onentrieschanged?.(bootEntries);
  }

  function handleDndFinalize(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;
    const { detail } = event;
    bootEntries = detail.items;
    // Apply the drag change with the captured start order
    ondragend?.(bootEntries);
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

    // Build context menu items
    const items = [
      {
        label: "Make Default",
        disabled: entry.is_default || busy || entry.id < 0,
        title: entry.is_default 
          ? "Already default" 
          : entry.id < 0 
            ? "Not possible for this entry" 
            : "",
        onclick: () => onmakedefault?.(entry)
      },
      {
        label: "Add Shortcut",
        disabled: busy || isPortable !== false,
        title: isPortable === true
          ? "Shortcuts are not available in portable mode"
          : isPortable === null
            ? "Loading portable mode status..."
            : "",
        onclick: () => onaddshortcut?.(entry)
      }
    ];

    // Create a temporary element at mouse position for Floating UI
    const tempTrigger = document.createElement('div');
    tempTrigger.style.position = 'fixed';
    tempTrigger.style.left = `${mouseEvent.clientX}px`;
    tempTrigger.style.top = `${mouseEvent.clientY}px`;
    tempTrigger.style.width = '1px';
    tempTrigger.style.height = '1px';
    tempTrigger.style.opacity = '0';
    tempTrigger.style.pointerEvents = 'none';
    document.body.appendChild(tempTrigger);

    // Toggle the global context menu
    openContextMenu({
      triggerElement: tempTrigger,
      items,
      preferredPlacement: 'right-start',
      onclose: () => {
        // Clean up temporary element when menu closes
        if (document.body.contains(tempTrigger)) {
          document.body.removeChild(tempTrigger);
        }
      },
      owner: 'boot-entries-right-click'
    });
  }

  // Close context menu when clicking elsewhere
  function handleDocumentClick() {
    // Context menu is now handled globally
  }
</script>

<svelte:document on:click={handleDocumentClick} />

<div class="flex-1 overflow-y-auto">
  <div class="mb-4">
    <h2 class="text-lg font-semibold pl-3 pb-1 max-w-2xl w-full mx-auto select-none">Boot Order</h2>
    <div
      class="flex flex-col gap-4 mb-2 bg-neutral-100 dark:bg-neutral-900 px-2 max-w-2xl w-full mx-auto"
      use:dndzone={{
        items: bootEntries,
        flipDurationMs: flipDuration,
        dropTargetStyle: {},

        dragDisabled: busy,
      }}
      on:consider={handleDndConsider}
      on:finalize={handleDndFinalize}
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
            onsetboottofirmwaresetup={onsetboottofirmwaresetup}
            onunsetboottofirmwaresetup={onunsetboottofirmwaresetup}
            onrestartnow={handleRestartNow}
            onremovefrombootorder={onremovefrombootorder}
            oncontextmenu={handleContextMenu}
          />
        </div>
      {/each}
    </div>
  </div>

  {#if others.length > 0}
    <details class="mb-4">
      <summary class="text-lg font-semibold cursor-pointer pl-3 pb-1 max-w-2xl w-full mx-auto select-none">Others</summary>
      <div class="flex flex-col gap-4 mb-2 bg-neutral-100 dark:bg-neutral-900 px-2 max-w-2xl w-full mx-auto">
        {#if discoveredEntriesLoading}
          <!-- Show EFI Setup immediately -->
          {#each others.filter(entry => entry.id === -200) as entry, idx}
            <BootEntryItem
              {entry}
              index={idx}
              totalEntries={others.length}
              {busy}
              isInOthers={true}
              onaddtobootorder={onaddtobootorder}
              onsetbootnext={onsetbootnext}
              onunsetbootnext={onunsetbootnext}
              onsetboottofirmwaresetup={onsetboottofirmwaresetup}
              onunsetboottofirmwaresetup={onunsetboottofirmwaresetup}
              onrestartnow={onrestartnow}
              oncontextmenu={handleContextMenu}
            />
          {/each}
          <!-- Show loading for other entries -->
          <div class="flex items-center justify-center p-8 text-neutral-500 dark:text-neutral-400">
            <div class="animate-spin rounded-full h-6 w-6 border-b-2 border-neutral-500 mr-2"></div>
            Loading boot entries...
          </div>
        {:else}
          {#each others as entry, idx (entry.id)}
            <BootEntryItem
              {entry}
              index={idx}
              totalEntries={others.length}
              {busy}
              isInOthers={true}
              onaddtobootorder={onaddtobootorder}
              onsetbootnext={onsetbootnext}
              onunsetbootnext={onunsetbootnext}
              onsetboottofirmwaresetup={onsetboottofirmwaresetup}
              onunsetboottofirmwaresetup={onunsetboottofirmwaresetup}
              onrestartnow={onrestartnow}
              oncontextmenu={handleContextMenu}
            />
          {/each}
        {/if}
      </div>
    </details>
  {/if}
</div>

<style>
  /* Remove white border/background from dragged element */
  :global(#dnd-action-dragged-el) {
    outline: none !important;
  }
</style>
