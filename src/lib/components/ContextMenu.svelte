<script lang="ts">
  import ContextMenuItem from "./ContextMenuItem.svelte";
  import { contextMenuStore, closeContextMenu, type ContextMenuState } from "../stores/contextMenu";

  // Subscribe to global context menu state
  let state: ContextMenuState = { show: false, x: 0, y: 0, items: [], onclose: () => {} };
  const unsubscribe = contextMenuStore.subscribe(value => {
    state = value;
  });

  // Cleanup subscription on destroy
  import { onDestroy } from 'svelte';
  onDestroy(unsubscribe);

  function handleMenuClick(event: MouseEvent) {
    event.stopPropagation();
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      closeContextMenu();
    }
  }
</script>

<svelte:document on:click={closeContextMenu} />

{#if state.show}
  <div
    class="fixed w-max flex flex-col bg-white dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-600 rounded-lg shadow-lg py-2 z-50"
    style="left: {state.x}px; top: {state.y}px;"
    onclick={handleMenuClick}
    onkeydown={handleKeyDown}
    role="menu"
    tabindex="-1"
  >
    {#each state.items as item}
      <ContextMenuItem
        disabled={item.disabled}
        title={item.title}
        onclick={() => {
          item.onclick();
          closeContextMenu();
        }}
      >
        {item.label}
      </ContextMenuItem>
    {/each}
  </div>
{/if}