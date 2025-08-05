<script lang="ts">
  import type { BootEntry } from "../types";
  import Button from "./Button.svelte";
  import Checkbox from "./Checkbox.svelte";

  export let entry: BootEntry;
  export let visible: boolean = false;
  export let oncreate:
    | ((config: {
        name: string;
        setBootNext: boolean;
        reboot: boolean;
      }) => void)
    | undefined = undefined;
  export let oncancel: (() => void) | undefined = undefined;

  if (entry.description === "Windows Boot Manager") {
    entry.description = "Windows";
  }
  let shortcutName = `Reboot to ${entry.description}`;
  let setBootNext = true; // Always checked and disabled
  let reboot = true;
  let originalName = `Reboot to ${entry.description}`;
  let hasUserChangedName = false;

  // Track if user manually changed the name
  function handleNameInput() {
    hasUserChangedName =
      shortcutName !== originalName &&
      shortcutName !== `BootNext ${entry.description}`;
  }

  // Update name when reboot checkbox changes (if user hasn't manually changed it)
  function handleRebootChange() {
    if (!hasUserChangedName) {
      shortcutName = reboot
        ? `Reboot to ${entry.description}`
        : `BootNext ${entry.description}`;
      originalName = shortcutName;
    }
  }

  function handleCreate() {
    oncreate?.({
      name: shortcutName,
      setBootNext,
      reboot,
    });
    visible = false;
  }

  function handleCancel() {
    oncancel?.();
    visible = false;
  }

  // Reset form when dialog opens
  let lastEntry: BootEntry | null = null;
  $: if (visible && entry && entry !== lastEntry) {
    shortcutName = `Reboot to ${entry.description}`;
    originalName = shortcutName;
    setBootNext = true;
    reboot = true;
    hasUserChangedName = false;
    lastEntry = entry;
  }

  // Close dialog when clicking outside
  function handleOverlayClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      handleCancel();
    }
  }

  // Close dialog when pressing Escape
  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      handleCancel();
    }
  }
</script>

{#if visible}
  <div
    class="fixed inset-0 bg-black/20 backdrop-blur-sm flex items-center justify-center z-50"
    on:click={handleOverlayClick}
    on:keydown={handleKeydown}
    role="dialog"
    aria-modal="true"
    aria-labelledby="dialog-title"
    tabindex="-1"
  >
    <div
      class="bg-white dark:bg-neutral-800 rounded-lg shadow-xl p-6 w-96 max-w-[90vw]"
    >
      <h2
        id="dialog-title"
        class="text-lg font-semibold mb-4 text-neutral-900 dark:text-neutral-100"
      >
        Create Shortcut
      </h2>

      <div class="space-y-4">
        <!-- Shortcut Name -->
        <div>
          <label
            for="shortcut-name"
            class="block text-sm font-medium mb-2 text-neutral-700 dark:text-neutral-300"
          >
            Shortcut Name
          </label>
          <input
            id="shortcut-name"
            type="text"
            bind:value={shortcutName}
            on:input={handleNameInput}
            class="w-full px-3 py-2 border border-neutral-300 dark:border-neutral-600 rounded-md bg-white dark:bg-neutral-700 text-neutral-900 dark:text-neutral-100 focus:outline-none focus:ring-2 focus:ring-sky-500"
          />
        </div>

        <!-- Set as BootNext (always checked and disabled) -->
        <Checkbox
          id="set-bootnext"
          bind:checked={setBootNext}
          disabled={true}
          label="Set as BootNext"
        />

        <!-- Reboot -->
        <Checkbox
          id="reboot"
          bind:checked={reboot}
          onchange={handleRebootChange}
          label="Reboot"
        />
      </div>

      <!-- Buttons -->
      <div class="flex justify-end space-x-3 mt-6">
        <Button variant="secondary" size="medium" onclick={handleCancel}>
          Cancel
        </Button>
        <Button variant="primary" size="medium" onclick={handleCreate}>
          Create Shortcut
        </Button>
      </div>
    </div>
  </div>
{/if}
