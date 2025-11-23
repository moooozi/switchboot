<script lang="ts">
  import { fade, scale } from "svelte/transition";

  export let visible: boolean = false;
  export let status: string = "";
  export let progress: number = 0;
  export let totalSize: number = 0;
</script>

{#if visible}
  <div
    class="fixed inset-0 bg-black/20 backdrop-blur-sm flex items-center justify-center z-50"
    role="dialog"
    aria-modal="true"
    aria-labelledby="update-dialog-title"
    tabindex="-1"
    transition:fade={{ duration: 100 }}
  >
    <div
      class="bg-white dark:bg-neutral-800 rounded-lg shadow-xl p-6 w-96 max-w-[90vw]"
      transition:scale={{ duration: 200, start: 0.95 }}
    >
      <h2
        id="update-dialog-title"
        class="text-lg font-semibold mb-4 text-neutral-900 dark:text-neutral-100"
      >
        Updating Switchboot
      </h2>

      <div class="space-y-4">
        <div class="text-sm text-neutral-600 dark:text-neutral-400">
          {status}
        </div>

        <div class="w-full bg-neutral-200 dark:bg-neutral-700 rounded-full h-2">
          <div
            class="bg-blue-600 h-2 rounded-full transition-all duration-300"
            style="width: {totalSize > 0
              ? Math.min((progress / totalSize) * 100, 100)
              : 0}%"
          ></div>
        </div>
      </div>
    </div>
  </div>
{/if}
