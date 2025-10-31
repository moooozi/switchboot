<script lang="ts">
  import { toggleContextMenu as toggleGlobalContextMenu } from "../stores/contextMenu";
  import { undoRedoStore } from "../stores/undoRedo";
  import Button from "./Button.svelte";

  export let changed: boolean;
  export let busy: boolean;
  export let onsave: (() => void) | undefined = undefined;
  export let ondiscard: (() => void) | undefined = undefined;
  export let onundo: (() => void) | undefined = undefined;
  export let onredo: (() => void) | undefined = undefined;

  let undoRedoState = { canUndo: false, canRedo: false, undoDescription: '', redoDescription: '' };

  // Subscribe to undo/redo state
  undoRedoStore.subscribe(state => {
    undoRedoState = state;
  });

  function toggleContextMenu(event: MouseEvent) {
    event.stopPropagation(); // Prevent the click from bubbling to document

    const button = event.currentTarget as HTMLElement;

    // Build context menu items
    const items = [
      {
        label: undoRedoState.canUndo ? `Undo ${undoRedoState.undoDescription}` : "Undo",
        disabled: !undoRedoState.canUndo || busy,
        onclick: () => onundo?.()
      },
      {
        label: undoRedoState.canRedo ? `Redo ${undoRedoState.redoDescription}` : "Redo",
        disabled: !undoRedoState.canRedo || busy,
        onclick: () => onredo?.()
      }
    ];

    // Toggle the global context menu
    toggleGlobalContextMenu('header-3dot', {
      triggerElement: button,
      items,
      preferredPlacement: 'bottom-start',
      onclose: () => {} // No special close handling needed
    });
  }
</script>

<div
  class="container max-w-2xl mx-auto flex items-center justify-between mb-8 px-3"
>
  <h1 class="text-3xl font-bold tracking-tight select-none">Switchboot</h1>
  <div class="flex gap-3 items-center">
    <Button
      variant="primary"
      size="large"
      disabled={!changed || busy}
      title="Save the current boot order"
      onclick={onsave}
    >
      Save
    </Button>
    <Button
      variant="secondary"
      size="large"
      disabled={!changed || busy}
      title="Discard boot order changes"
      onclick={ondiscard}
    >
      Discard
    </Button>
    
    <!-- 3 dots menu button -->
    <div class="relative">
      <button
        class="w-10 h-10 flex items-center justify-center rounded-lg
               hover:bg-neutral-200 dark:hover:bg-neutral-700
               transition-colors disabled:opacity-50"
        disabled={busy}
        title="More options"
        aria-label="More options"
        onclick={toggleContextMenu}
      >
        <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
          <path d="M10 6a2 2 0 110-4 2 2 0 010 4zM10 12a2 2 0 110-4 2 2 0 010 4zM10 18a2 2 0 110-4 2 2 0 010 4z"/>
        </svg>
      </button>
    </div>
  </div>
</div>
