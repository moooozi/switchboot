<script lang="ts">
  export let variant:
    | "primary"
    | "secondary"
    | "neutral"
    | "emerald"
    | "amber"
    | "purple"
    | "small-neutral" = "primary";
  export let size: "small" | "medium" | "large" = "medium";
  export let disabled: boolean = false;
  export let title: string = "";
  export let ariaLabel: string = "";
  export let onclick: (() => void) | undefined = undefined;
  export let rounded: "full" | "normal" | "circle" = "full";

  // Build classes using CSS variants from app.css
  $: roundedClass =
    rounded === "full"
      ? "rounded-full"
      : rounded === "circle"
        ? "rounded-full"
        : "rounded";

  $: sizeClass = rounded === "circle" ? `size-circle-${size}` : `size-${size}`;

  $: variantClass = `btn-${variant}`;

  $: classes = `btn ${variantClass} ${sizeClass} ${roundedClass} select-none`;

  function handleClick() {
    if (!disabled) {
      onclick?.();
    }
  }
</script>

<button
  class={classes}
  {disabled}
  {title}
  aria-label={ariaLabel}
  on:click={handleClick}
>
  <slot />
</button>
