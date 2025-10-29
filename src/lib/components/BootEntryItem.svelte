<script lang="ts">
  import type { BootEntry } from "../types";
  import Button from "./Button.svelte";
  import Label from "./Label.svelte";
  import OsIcon from "./OsIcon.svelte";

  export let entry: BootEntry;
  export let index: number;
  export let totalEntries: number;
  export let busy: boolean;
  export let isInOthers = false;

  export let onmoveup: ((index: number) => void) | undefined = undefined;
  export let onmovedown: ((index: number) => void) | undefined = undefined;
  export let onsetbootnext: ((entry: BootEntry) => void) | undefined =
    undefined;
  export let onunsetbootnext: (() => void) | undefined = undefined;
  export let onsetboottofirmwaresetup: (() => void) | undefined = undefined;
  export let onunsetboottofirmwaresetup: (() => void) | undefined = undefined;
  export let onrestartnow: (() => void) | undefined = undefined;
  export let onaddtobootorder: ((entry: BootEntry) => void) | undefined =
    undefined;
  export let onremovefrombootorder: ((entry: BootEntry) => void) | undefined =
    undefined;
  export let oncontextmenu:
    | ((data: { entry: BootEntry; mouseEvent: MouseEvent }) => void)
    | undefined = undefined;

  function handleContextMenu(event: MouseEvent) {
    oncontextmenu?.({ entry, mouseEvent: event });
  }
</script>

<div
  class={`flex items-center gap-3 p-4 rounded-xl border transition-colors select-none ${isInOthers ? 'cursor-default' : 'cursor-grab'}
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
      onclick={() => {
        if (entry.id === -200) {
          onunsetboottofirmwaresetup?.();
        } else {
          onunsetbootnext?.();
        }
      }}
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
      onclick={() => {
        if (entry.id === -200) {
          onsetboottofirmwaresetup?.();
        } else {
          onsetbootnext?.(entry);
        }
      }}
    >
      BootNext
    </Button>
  {/if}

  {#if isInOthers}
    <Button
      variant="neutral"
      size="small"
      rounded="circle"
      disabled={busy || entry.id < 0}
      ariaLabel="Add to boot order"
      title="Add this entry to the boot order"
      onclick={() => onaddtobootorder?.(entry)}
    >
      <span class="text-sm">
        <!-- Custom "+" symbol using CSS - Unicode characters had centering issues due to font metrics -->
        <div class="w-3 h-3 relative">
          <div class="absolute inset-0 w-0.5 bg-current mx-auto"></div>
          <div class="absolute inset-0 h-0.5 bg-current my-auto"></div>
        </div>
      </span>
    </Button>
  {:else}
    <Button
      variant="neutral"
      size="small"
      rounded="circle"
      disabled={index === 0 || busy || entry.id < 0}
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
      disabled={index === totalEntries - 1 || busy || entry.id < 0}
      ariaLabel="Move down"
      title="Move entry down"
      onclick={() => onmovedown?.(index)}
    >
      ↓
    </Button>
    <Button
      variant="neutral"
      size="small"
      rounded="circle"
      disabled={busy || entry.id < 0}
      ariaLabel="Remove from boot order"
      title="Remove this entry from the boot order"
      onclick={() => onremovefrombootorder?.(entry)}
    >
      <span class="text-sm">✕</span>
    </Button>
  {/if}
</div>
