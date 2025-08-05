<script lang="ts">
  export let disabled: boolean = false;
  export let onclick: (() => void) | undefined = undefined;
  export let title: string = "";
</script>

<button
  class="w-auto text-left px-4 py-2 transition-colors
    hover:bg-neutral-100 dark:hover:bg-neutral-700
    disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent"
  {disabled}
  {title}
  on:click={() => {
    if (!disabled) onclick?.();
  }}
>
  <slot />
</button>
