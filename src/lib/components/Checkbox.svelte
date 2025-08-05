<script lang="ts">
  export let checked: boolean = false;
  export let disabled: boolean = false;
  export let label: string = "";
  export let id: string = "";
  export let onchange: ((event: Event) => void) | undefined = undefined;

  function handleChange(event: Event) {
    if (!disabled) {
      onchange?.(event);
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if ((event.key === "Enter" || event.key === " ") && !disabled) {
      event.preventDefault();
      checked = !checked;
      // Create a synthetic event for consistency
      const syntheticEvent = new Event("change");
      handleChange(syntheticEvent);
    }
  }

  function handleClick() {
    if (!disabled) {
      checked = !checked;
      // Create a synthetic event for consistency
      const syntheticEvent = new Event("change");
      handleChange(syntheticEvent);
    }
  }
</script>

<div class="flex items-center space-x-3">
  <!-- Hidden native checkbox for form compatibility -->
  <input
    {id}
    type="checkbox"
    bind:checked
    {disabled}
    on:change={handleChange}
    class="sr-only"
    tabindex="-1"
  />

  <!-- Custom checkbox visual -->
  <div
    class="relative w-6 h-6 rounded border-2 transition-all duration-200
      {disabled ? 'opacity-60 cursor-not-allowed' : 'cursor-pointer'}
      {checked
      ? 'bg-blue-600 border-blue-600 dark:bg-blue-500 dark:border-blue-500'
      : 'bg-white dark:bg-neutral-800 border-neutral-300 dark:border-neutral-600'}
      {!disabled && !checked
      ? 'hover:border-blue-500 dark:hover:border-blue-400'
      : ''}
      {!disabled
      ? 'focus-within:ring-2 focus-within:ring-blue-500 focus-within:ring-offset-0'
      : ''}"
    on:click={handleClick}
    on:keydown={handleKeydown}
    role="checkbox"
    aria-checked={checked}
    aria-disabled={disabled}
    tabindex={disabled ? -1 : 0}
  >
    <!-- Checkmark -->
    {#if checked}
      <div class="absolute inset-0 flex items-center justify-center">
        <div
          class="w-3 h-2 border-l-2 border-b-2 border-white transform rotate-[-45deg] translate-y-[-1px]"
        ></div>
      </div>
    {/if}
  </div>

  <span
    class="text-sm text-neutral-700 dark:text-neutral-300 {disabled
      ? 'opacity-60 cursor-not-allowed'
      : 'cursor-pointer'} select-none"
    on:click={handleClick}
    on:keydown={handleKeydown}
    role="button"
    tabindex={disabled ? -1 : 0}
  >
    {label}
  </span>
</div>
