<script lang="ts">
  import { dndzone } from "svelte-dnd-action";
  import { flip } from "svelte/animate";
  import { openContextMenu } from "../stores/contextMenu";
  import type { BootEntry } from "../types";
  import { CrossZoneDragManager } from "../utils/crossZoneDragManager";
  import BootEntryItem from "./BootEntryItem.svelte";
  import CollapsibleSection from "./CollapsibleSection.svelte";

  // Props
  export let bootEntries: BootEntry[];
  export let busy: boolean;
  export let isPortable: boolean | null;
  export let others: BootEntry[] = [];
  export let discoveredEntriesLoading = false;

  // Callbacks for boot entry actions
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

  // Constants
  const FLIP_DURATION_MS = 150;
  const DND_ZONE_TYPE = "boot-entries";

  // Drag manager instance
  const dragManager = new CrossZoneDragManager();

  // Local state for drag and drop (synced with props when not dragging)
  let localBootEntries: BootEntry[] = [];
  let localOthers: BootEntry[] = [];

  // Reactive sync: Update local state when props change and not dragging
  $: if (!dragManager.isActive()) {
    localBootEntries = bootEntries;
    localOthers = others;
  }

  // ==================== Drag Prevention ====================

  /**
   * Check if an entry can be dragged (entries with negative IDs cannot be dragged)
   */
  function isDraggable(entry: BootEntry): boolean {
    return entry.id >= 0;
  }

  // ==================== Boot Order Zone Handlers ====================

  function handleBootOrderConsider(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;

    // Initialize drag tracking on first consider event
    if (!dragManager.isActive()) {
      dragManager.startDrag(bootEntries, others);
      ondragstart?.();
    }

    localBootEntries = event.detail.items;
    onentrieschanged?.(localBootEntries);
  }

  function handleBootOrderFinalize(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;

    const finalItems = event.detail.items;
    const analysis = dragManager.analyzeBootOrderChanges(finalItems);

    // Update local and prop state
    bootEntries = finalItems;
    localBootEntries = finalItems;

    // Handle cross-zone additions (from Others to Boot Order)
    analysis.addedFromOthers.forEach(({ entry, position }) => {
      onaddtobootorder?.(entry, position);
    });

    // Handle cross-zone removals (from Boot Order to Others)
    analysis.removedToOthers.forEach(entry => {
      onremovefrombootorder?.(entry);
    });

    // Handle reordering within Boot Order only
    if (analysis.isReorderOnly) {
      ondragend?.(bootEntries);
    }

    dragManager.endDrag();
  }

  // ==================== Others Zone Handlers ====================

  function handleOthersConsider(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;

    // Initialize drag tracking on first consider event
    if (!dragManager.isActive()) {
      dragManager.startDrag(bootEntries, others);
    }

    localOthers = event.detail.items;
  }

  function handleOthersFinalize(event: CustomEvent<{ items: BootEntry[] }>) {
    if (busy) return;

    const finalItems = event.detail.items;
    const analysis = dragManager.analyzeOthersChanges(finalItems);

    // Update local state
    localOthers = finalItems;

    // Handle items moved from Boot Order to Others
    analysis.addedFromBoot.forEach(entry => {
      onremovefrombootorder?.(entry);
    });

    dragManager.endDrag();
  }

  // ==================== Action Handlers ====================

  const handleMoveUp = (index: number) => onmoveup?.(index);
  const handleMoveDown = (index: number) => onmovedown?.(index);
  const handleSetBootNext = (entry: BootEntry) => onsetbootnext?.(entry);
  const handleUnsetBootNext = () => onunsetbootnext?.();
  const handleRestartNow = () => onrestartnow?.();

  // ==================== Context Menu ====================

  function handleContextMenu(data: { entry: BootEntry; mouseEvent: MouseEvent }) {
    const { entry, mouseEvent } = data;
    mouseEvent.preventDefault();

    const contextMenuItems = [
      {
        label: "Make Default",
        disabled: entry.is_default || busy || entry.id < 0,
        title: entry.is_default
          ? "Already default"
          : entry.id < 0
            ? "Not possible for this entry"
            : "",
        onclick: () => onmakedefault?.(entry),
      },
      {
        label: "Add Shortcut",
        disabled: busy || isPortable !== false,
        title:
          isPortable === true
            ? "Shortcuts are not available in portable mode"
            : isPortable === null
              ? "Loading portable mode status..."
              : "",
        onclick: () => onaddshortcut?.(entry),
      },
    ];

    // Create temporary trigger element for positioning
    const trigger = createContextMenuTrigger(mouseEvent.clientX, mouseEvent.clientY);

    openContextMenu({
      triggerElement: trigger,
      items: contextMenuItems,
      preferredPlacement: "right-start",
      onclose: () => cleanupContextMenuTrigger(trigger),
      owner: "boot-entries-right-click",
    });
  }

  function createContextMenuTrigger(x: number, y: number): HTMLDivElement {
    const trigger = document.createElement("div");
    Object.assign(trigger.style, {
      position: "fixed",
      left: `${x}px`,
      top: `${y}px`,
      width: "1px",
      height: "1px",
      opacity: "0",
      pointerEvents: "none",
    });
    document.body.appendChild(trigger);
    return trigger;
  }

  function cleanupContextMenuTrigger(trigger: HTMLDivElement): void {
    if (document.body.contains(trigger)) {
      document.body.removeChild(trigger);
    }
  }
</script>

<div class="flex-1 overflow-y-auto">
  <CollapsibleSection title="Boot Order" count={bootEntries.length} open>
    <div
      class="flex flex-col gap-4"
      use:dndzone={{
        items: localBootEntries,
        flipDurationMs: FLIP_DURATION_MS,
        dropTargetStyle: {},
        dragDisabled: busy,
        type: DND_ZONE_TYPE,
      }}
      on:consider={handleBootOrderConsider}
      on:finalize={handleBootOrderFinalize}
    >
      {#each localBootEntries as entry, idx (entry.id)}
        <div 
          animate:flip={{ duration: FLIP_DURATION_MS }}
          class:non-draggable={!isDraggable(entry)}
        >
          <BootEntryItem
            {entry}
            index={idx}
            totalEntries={bootEntries.length}
            {busy}
            onmoveup={handleMoveUp}
            onmovedown={handleMoveDown}
            onsetbootnext={handleSetBootNext}
            onunsetbootnext={handleUnsetBootNext}
            {onsetboottofirmwaresetup}
            {onunsetboottofirmwaresetup}
            onrestartnow={handleRestartNow}
            {onremovefrombootorder}
            oncontextmenu={handleContextMenu}
          />
        </div>
      {/each}
    </div>
  </CollapsibleSection>

  {#if others.length > 0}
    <CollapsibleSection title="Others" count={others.length} open>
      <div
        class="flex flex-col gap-4"
        use:dndzone={{
          items: localOthers,
          flipDurationMs: FLIP_DURATION_MS,
          dropTargetStyle: {},
          dragDisabled: busy || discoveredEntriesLoading,
          type: DND_ZONE_TYPE,
        }}
        on:consider={handleOthersConsider}
        on:finalize={handleOthersFinalize}
      >
        {#if discoveredEntriesLoading}
          {#each localOthers.filter((entry) => entry.id === -200) as entry, idx (entry.id)}
            <div 
              animate:flip={{ duration: FLIP_DURATION_MS }}
              class:non-draggable={!isDraggable(entry)}
            >
              <BootEntryItem
                {entry}
                index={idx}
                totalEntries={others.length}
                {busy}
                isInOthers={true}
                {onaddtobootorder}
                {onsetbootnext}
                {onunsetbootnext}
                {onsetboottofirmwaresetup}
                {onunsetboottofirmwaresetup}
                {onrestartnow}
                oncontextmenu={handleContextMenu}
              />
            </div>
          {/each}
          <div
            class="flex items-center justify-center p-8 text-neutral-500 dark:text-neutral-400"
          >
            <div
              class="animate-spin rounded-full h-6 w-6 border-b-2 border-neutral-500 mr-2"
            ></div>
            Loading boot entries...
          </div>
        {:else}
          {#each localOthers as entry, idx (entry.id)}
            <div 
              animate:flip={{ duration: FLIP_DURATION_MS }}
              class:non-draggable={!isDraggable(entry)}
            >
              <BootEntryItem
                {entry}
                index={idx}
                totalEntries={others.length}
                {busy}
                isInOthers={true}
                {onaddtobootorder}
                {onsetbootnext}
                {onunsetbootnext}
                {onsetboottofirmwaresetup}
                {onunsetboottofirmwaresetup}
                {onrestartnow}
                oncontextmenu={handleContextMenu}
              />
            </div>
          {/each}
        {/if}
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
