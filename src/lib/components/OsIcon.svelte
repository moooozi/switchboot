<script lang="ts">
  export let description: string | undefined;
  export let size: number = 32;

  import { getIconId } from "../iconMap";

  const iconId = getIconId(description);
  let svgPath: string | undefined = iconId
    ? `/os-icons/${iconId}.svg`
    : undefined;
</script>

{#if svgPath}
  <img
    src={svgPath}
    alt={description}
    width={size}
    height={size}
    class="rounded-sm"
  />
{:else}
  <!-- preserve layout when no icon is available -->
  <div
    class="rounded-sm"
    aria-hidden="true"
    style="width: {size}px; height: {size}px; display: inline-block;"
  ></div>
{/if}
