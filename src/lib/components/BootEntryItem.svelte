<script lang="ts">
  import type { BootEntry } from "../types";
  import Button from "./Button.svelte";
  import Label from "./Label.svelte";
  import OsIcon from "./OsIcon.svelte";

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
  <div class="flex items-center gap-3 flex-1">
  <OsIcon description={entry.description} size={28} />
    <span class="text-base">{entry.description}</span>
  </div>

  {#if entry.is_current}
    <Label
      variant="purple"
      size="small"
      title="This entry was used to boot the current OS"
    >
      Current
    </Label>
  {/if}

  {#if entry.is_default}
    <Label
      variant="primary"
      size="small"
      title="This is the default firmware boot entry"
    >
      Default
    </Label>
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
