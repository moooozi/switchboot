<script lang="ts">
  import type { BootEntry } from "../types";
  import Button from "./Button.svelte";

  export let entry: BootEntry;
  export let index: number;
  export let totalEntries: number;
  export let busy: boolean;

  export let onmoveup: ((index: number) => void) | undefined = undefined;
  export let onmovedown: ((index: number) => void) | undefined = undefined;
  export let onsetbootnext: ((entry: BootEntry) => void) | undefined =
    undefined;
  export let onunsetbootnext: (() => void) | undefined = undefined;
  export let onrestartnow: (() => void) | undefined = undefined;
  export let oncontextmenu:
    | ((data: { entry: BootEntry; mouseEvent: MouseEvent }) => void)
    | undefined = undefined;

  function handleContextMenu(event: MouseEvent) {
    oncontextmenu?.({ entry, mouseEvent: event });
  }
</script>

<div
  class={`flex items-center gap-3 p-4 rounded-xl border transition-colors cursor-grab
    ${
      entry.is_default
        ? "border-sky-500"
        : entry.is_bootnext
          ? "border-emerald-500"
          : "border-neutral-200 dark:border-neutral-700"
    }
    bg-neutral-200 dark:bg-neutral-800`}
  data-id={entry.id}
  on:contextmenu={handleContextMenu}
  role="button"
  tabindex="0"
>
  <span class="flex-1 text-base">{entry.description}</span>

  {#if entry.is_current}
    <span
      class="bg-purple-300 text-purple-800 dark:bg-purple-600 dark:text-white text-xs font-semibold px-2 py-1 rounded-full mr-2 select-none opacity-70"
      title="This entry was used to boot the current OS"
    >
      Current
    </span>
  {/if}

  {#if entry.is_default}
    <Button
      variant="primary"
      size="small"
      disabled={true}
      title="This is the default firmware boot entry"
    >
      Default
    </Button>
  {:else if entry.is_bootnext}
    <Button
      variant="emerald"
      size="small"
      disabled={busy}
      title="Unset BootNext (cancel one-time boot override)"
      onclick={() => onunsetbootnext?.()}
    >
      Undo
    </Button>
    <Button
      variant="amber"
      size="small"
      disabled={busy}
      title="Restart now to boot this entry next"
      onclick={() => onrestartnow?.()}
    >
      Restart
    </Button>
  {:else}
    <Button
      variant="small-neutral"
      size="small"
      disabled={busy}
      title="Set this entry as BootNext (one-time boot override)"
      onclick={() => onsetbootnext?.(entry)}
    >
      BootNext
    </Button>
  {/if}

  <Button
    variant="neutral"
    size="small"
    rounded="circle"
    disabled={index === 0 || busy}
    ariaLabel="Move up"
    title="Move entry up"
    onclick={() => onmoveup?.(index)}
  >
    ↑
  </Button>
  <Button
    variant="neutral"
    size="small"
    rounded="circle"
    disabled={index === totalEntries - 1 || busy}
    ariaLabel="Move down"
    title="Move entry down"
    onclick={() => onmovedown?.(index)}
  >
    ↓
  </Button>
</div>
