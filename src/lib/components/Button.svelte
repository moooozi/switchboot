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

  $: baseClasses = `transition font-medium focus:outline-none focus:ring-2 focus:ring-offset-0 ${rounded === "full" ? "rounded-full" : rounded === "circle" ? "rounded-full" : "rounded"}`;

  function getSizeClasses(size: string) {
    switch (size) {
      case "small":
        return "px-3 py-1 text-xs";
      case "medium":
        return "px-4 py-2 text-sm";
      case "large":
        return "px-6 py-2 text-base";
      default:
        return "px-4 py-2 text-sm";
    }
  }

  function getCircleSizeClasses(size: string) {
    switch (size) {
      case "small":
        return "w-8 h-8 text-base flex items-center justify-center";
      case "medium":
        return "w-10 h-10 text-lg flex items-center justify-center";
      case "large":
        return "w-12 h-12 text-xl flex items-center justify-center";
      default:
        return "w-8 h-8 text-base flex items-center justify-center";
    }
  }

  function getVariantClasses(variant: string = "primary") {
    switch (variant) {
      case "primary":
        return "bg-blue-600 text-white hover:bg-blue-700 dark:bg-blue-500 dark:hover:bg-blue-700 focus:ring-blue-500";
      case "secondary":
        return "bg-neutral-200 text-neutral-800 hover:bg-neutral-300 dark:bg-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-600 focus:ring-neutral-500";
      case "neutral":
        return "bg-neutral-300 dark:bg-neutral-700 text-neutral-700 dark:text-neutral-200 hover:brightness-95 dark:hover:bg-neutral-600 focus:ring-neutral-500";
      case "emerald":
        return "bg-emerald-500 text-white hover:bg-emerald-600 dark:bg-emerald-700 dark:hover:bg-emerald-600 focus:ring-emerald-500";
      case "amber":
        return "bg-amber-500 text-white hover:bg-amber-600 dark:bg-amber-700 dark:hover:bg-amber-600 focus:ring-amber-500";
      case "purple":
        return "bg-purple-300 text-purple-800 hover:bg-purple-400 dark:bg-purple-600 dark:text-white dark:hover:bg-purple-700 focus:ring-purple-500 opacity-70";
      case "small-neutral":
        return "bg-neutral-300 dark:bg-neutral-600 text-neutral-800 dark:text-neutral-200 hover:bg-sky-500 hover:text-white dark:hover:bg-sky-600 focus:ring-sky-500";
    }
  }

  $: sizeClasses =
    rounded === "circle" ? getCircleSizeClasses(size) : getSizeClasses(size);
  $: variantClasses = getVariantClasses(variant);

  $: disabledClasses =
    "cursor-pointer disabled:cursor-not-allowed disabled:bg-neutral-300 disabled:text-neutral-400 dark:disabled:bg-neutral-700 dark:disabled:text-neutral-500 disabled:shadow-none disabled:hover:bg-neutral-300 dark:disabled:hover:bg-neutral-700 disabled:hover:text-neutral-400 dark:disabled:hover:text-neutral-500";

  function handleClick() {
    if (!disabled) {
      onclick?.();
    }
  }
</script>

<button
  class="{baseClasses} {sizeClasses} {variantClasses} {disabledClasses}"
  {disabled}
  {title}
  aria-label={ariaLabel}
  on:click={handleClick}
>
  <slot />
</button>
