<script lang="ts">
  export let description: string | undefined;
  export let size: number = 32;

  import { getIconId } from "../iconMap";

  const iconId = getIconId(description);
  let svgPath = `/os-icons/${iconId}.svg`;
</script>

<img src={svgPath} alt={description} width={size} height={size} class="rounded-sm" />
