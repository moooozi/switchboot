<script lang="ts">
  export let variant:
    | "primary"
    | "secondary"
    | "neutral"
    | "emerald"
    | "amber"
    | "purple"
    | "small-neutral" = "primary";
  export let size: "small" | "medium" | "large" = "small";
  export let title: string = "";
  export let rounded: "full" | "normal" = "full";

  // Build classes using CSS variants from app.css
  $: roundedClass = rounded === "full" ? "rounded-full" : "rounded";
  $: sizeClass = `size-${size}-label`;
  $: variantClass = `label-${variant}`;
  $: classes = `label ${variantClass} ${sizeClass} ${roundedClass}`;
</script>

<span class={classes} {title}>
  <slot />
</span>
