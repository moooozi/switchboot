<script lang="ts">
  /**
   * BootEntriesList Component
   * 
   * Manages two drag-and-drop zones for boot entries:
   * - Boot Order: Ordered list of bootable entries with full reordering support
   * - Others: Unordered discovered entries that can be added to boot order
   */

  import { dndzone } from "svelte-dnd-action";
  import { flip } from "svelte/animate";
  import type { BootEntry } from "../types";
  import { CrossZoneDragManager } from "../utils/crossZoneDragManager";
  import { smoothHeight } from "../utils/smoothHeight";
  import { isDraggable, haveSameItems, createDndConfig } from "../utils/bootEntryHelpers";
  import { openBootEntryContextMenu } from "./BootEntriesContextMenu";
  import BootEntryItem from "./BootEntryItem.svelte";
  import CollapsibleSection from "./CollapsibleSection.svelte";

  // ==================== Props ====================
  
  export let bootEntries: BootEntry[];
  export let busy: boolean;
  export let isPortable: boolean | null;
  export let others: BootEntry[] = [];
  export let discoveredEntriesLoading = false;

  // Event callbacks
  export let onentrieschanged: ((entries: BootEntry[]) => void) | undefined = undefined;
  export let ondragstart: (() => void) | undefined = undefined;
  export let ondragend: ((entries: BootEntry[]) => void) | undefined = undefined;
  export let onmoveup: ((index: number) => void) | undefined = undefined;
  export let onmovedown: ((index: number) => void) | undefined = undefined;
  export let onsetbootnext: ((entry: BootEntry) => void) | undefined = undefined;
  export let onunsetbootnext: (() => void) | undefined = undefined;
  export let onsetboottofirmwaresetup: (() => void) | undefined = undefined;
  export let onunsetboottofirmwaresetup: (() => void) | undefined = undefined;
  export let onrestartnow: (() => void) | undefined = undefined;
  export let onmakedefault: ((entry: BootEntry) => void) | undefined = undefined;
  export let onaddshortcut: ((entry: BootEntry) => void) | undefined = undefined;
  export let onaddtobootorder: ((entry: BootEntry, position?: number) => void) | undefined = undefined;
  export let onremovefrombootorder: ((entry: BootEntry) => void) | undefined = undefined;

  // ==================== Constants & State ====================
  
  const FLIP_DURATION_MS = 150;
  const DND_ZONE_TYPE = "boot-entries";
  const dragManager = new CrossZoneDragManager();

  let localBootEntries: BootEntry[] = [];
  let localOthers: BootEntry[] = [];

  $: if (!dragManager.isActive()) {
    localBootEntries = bootEntries;
    localOthers = others;
  }

  // ==================== Drag & Drop Handlers ====================

  function handleBootOrderConsider(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;

    if (!dragManager.isActive()) {
      dragManager.startDrag(bootEntries, others);
    }

    localBootEntries = event.detail.items;
    onentrieschanged?.(localBootEntries);
  }

  function handleBootOrderFinalize(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;

    const finalItems = event.detail.items;
    const analysis = dragManager.analyzeBootOrderChanges(finalItems);

    bootEntries = localBootEntries = finalItems;

    // Trigger appropriate callbacks based on drag operation type
    analysis.addedFromOthers.forEach(({ entry, position }) => onaddtobootorder?.(entry, position));
    analysis.removedToOthers.forEach(entry => onremovefrombootorder?.(entry));

    // Only record reorder operations (cross-zone ops create their own undo events)
    if (analysis.isReorderOnly) {
      ondragstart?.();
      ondragend?.(bootEntries);
    }

    dragManager.endDrag();
  }

  function handleOthersConsider(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;

    if (!dragManager.isActive()) {
      dragManager.startDrag(bootEntries, others);
    }

    // Prevent reordering within Others zone - only allow cross-zone additions
    if (haveSameItems(localOthers, event.detail.items)) return;
    
    localOthers = event.detail.items;
  }

  function handleOthersFinalize(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;

    const analysis = dragManager.analyzeOthersChanges(event.detail.items);
    localOthers = event.detail.items;

    analysis.addedFromBoot.forEach(entry => onremovefrombootorder?.(entry));
    dragManager.endDrag();
  }

  // ==================== Entry Action Handlers ====================

  const handleMoveUp = (index: number) => onmoveup?.(index);
  const handleMoveDown = (index: number) => onmovedown?.(index);
  const handleSetBootNext = (entry: BootEntry) => onsetbootnext?.(entry);
  const handleUnsetBootNext = () => onunsetbootnext?.();
  const handleRestartNow = () => onrestartnow?.();

  const handleContextMenu = ({ entry, mouseEvent }: { entry: BootEntry; mouseEvent: MouseEvent }) => {
    openBootEntryContextMenu({
      entry,
      mouseEvent,
      busy,
      isPortable,
      onMakeDefault: onmakedefault,
      onAddShortcut: onaddshortcut,
    });
  };

  const getEntryProps = (entry: BootEntry, idx: number, isInOthers = false) => ({
    entry,
    index: idx,
    totalEntries: isInOthers ? others.length : bootEntries.length,
    busy,
    isInOthers,
    oncontextmenu: handleContextMenu,
    onsetbootnext: handleSetBootNext,
    onunsetbootnext: handleUnsetBootNext,
    onsetboottofirmwaresetup,
    onunsetboottofirmwaresetup,
    onrestartnow: handleRestartNow,
    ...(isInOthers 
      ? { onaddtobootorder } 
      : { onmoveup: handleMoveUp, onmovedown: handleMoveDown, onremovefrombootorder }
    ),
  });

  const dndConfig = (items: BootEntry[], disabled = false) => 
    createDndConfig(items, disabled, FLIP_DURATION_MS, DND_ZONE_TYPE);
</script>

<div class="flex-1 overflow-y-scroll">
  <!-- Boot Order Section -->
  <CollapsibleSection title="Boot Order" count={bootEntries.length} open>
    <div use:smoothHeight>
      <div
        class="flex flex-col gap-4"
        use:dndzone={dndConfig(localBootEntries, busy)}
        on:consider={handleBootOrderConsider}
        on:finalize={handleBootOrderFinalize}
      >
        {#each localBootEntries as entry, idx (entry.id)}
          <div 
            animate:flip={{ duration: FLIP_DURATION_MS }}
            class:non-draggable={!isDraggable(entry, localBootEntries.length === 1)}
          >
            <BootEntryItem {...getEntryProps(entry, idx)} />
          </div>
        {/each}
      </div>
    </div>
  </CollapsibleSection>

  <!-- Others Section -->
  {#if others.length > 0}
    <CollapsibleSection title="Others" count={others.length} open>
      <div use:smoothHeight>
        <div
          class="flex flex-col gap-4"
          use:dndzone={{ ...dndConfig(localOthers, busy || discoveredEntriesLoading), dropFromOthersDisabled: false }}
          on:consider={handleOthersConsider}
          on:finalize={handleOthersFinalize}
        >
          {#if discoveredEntriesLoading}
            <!-- Show special entries during loading -->
            {#each localOthers.filter(e => e.id === -200) as entry, idx (entry.id)}
              <div class:non-draggable={!isDraggable(entry, false)}>
                <BootEntryItem {...getEntryProps(entry, idx, true)} />
              </div>
            {/each}
            
            <!-- Loading indicator -->
            <div class="flex items-center justify-center p-8 text-neutral-500 dark:text-neutral-400">
              <div class="animate-spin rounded-full h-6 w-6 border-b-2 border-neutral-500 mr-2"></div>
              Loading boot entries...
            </div>
          {:else}
            {#each localOthers as entry, idx (entry.id)}
              <div class:non-draggable={!isDraggable(entry, false)}>
                <BootEntryItem {...getEntryProps(entry, idx, true)} />
              </div>
            {/each}
          {/if}
        </div>
      </div>
    </CollapsibleSection>
  {/if}
</div>

<style>
  /* Remove white border/background from dragged element */
  :global(#dnd-action-dragged-el) {
    outline: none !important;
  }

  /* Prevent dragging for items with negative IDs */
  .non-draggable {
    pointer-events: none;
    user-select: none;
    cursor: default !important;
  }
</style>
