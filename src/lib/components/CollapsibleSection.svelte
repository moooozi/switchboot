<script lang="ts">
  import { slide } from 'svelte/transition';

  export let title: string;
  export let count: number | undefined = undefined;
  export let open = false;
  export let showCount = true;
</script>

<div class="mb-4 max-w-2xl w-full mx-auto">
  <button
    type="button"
    class="flex items-center gap-2 text-lg font-semibold cursor-pointer px-3 py-2 
           rounded-lg transition-all duration-200
           hover:bg-neutral-200/50 dark:hover:bg-neutral-800/50
           select-none w-full text-left"
    on:click={() => open = !open}
  >
    <svg
      class="w-5 h-5 transition-transform duration-200 {open ? 'rotate-90' : ''}"
      fill="none"
      stroke="currentColor"
      viewBox="0 0 24 24"
    >
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2"
        d="M9 5l7 7-7 7"
      />
    </svg>
    <span>{title}</span>
    {#if showCount && count !== undefined}
      <span
        class="ml-auto text-sm font-normal text-neutral-500 dark:text-neutral-400"
      >
        {count}
        {count === 1 ? "entry" : "entries"}
      </span>
    {/if}
  </button>
  <div
    class="flex flex-col gap-4 mt-3 mb-2 px-2"
  >
    {#if open}
      <div transition:slide={{ duration: 300 }}>
        <slot />
      </div>
    {/if}
  </div>
</div>
