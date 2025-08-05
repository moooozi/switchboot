<script lang="ts">
  import Button from "./Button.svelte";

  export let changed: boolean;
  export let busy: boolean;
  export let onsave: (() => void) | undefined = undefined;
  export let ondiscard: (() => void) | undefined = undefined;
</script>

<div
  class="container max-w-2xl mx-auto flex items-center justify-between mb-8 px-3"
>
  <h1 class="text-3xl font-bold tracking-tight select-none">Switchboot</h1>
  <div class="flex gap-3">
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
  </div>
</div>
