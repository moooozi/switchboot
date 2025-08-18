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

  $: baseClasses = `font-semibold select-none border ${rounded === "full" ? "rounded-full" : "rounded"}`;

  function getSizeClasses(size: string) {
    switch (size) {
      case "small":
        return "px-2 py-1 text-xs";
      case "medium":
        return "px-3 py-1.5 text-sm";
      case "large":
        return "px-4 py-2 text-base";
      default:
        return "px-2 py-1 text-xs";
    }
  }

  function getVariantClasses(variant: string = "primary") {
    switch (variant) {
      case "primary":
        return "bg-blue-100 text-blue-700 border-blue-600 dark:bg-blue-900 dark:text-blue-300 dark:border-blue-400";
      case "secondary":
        return "bg-neutral-100 text-neutral-700 border-neutral-200 dark:bg-neutral-900 dark:text-neutral-200 dark:border-neutral-500";
      case "neutral":
        return "bg-neutral-50 text-neutral-600 border-neutral-500 dark:bg-neutral-900 dark:text-neutral-300 dark:border-neutral-600";
      case "emerald":
        return "bg-emerald-100 text-emerald-700 border-emerald-600 dark:bg-emerald-900 dark:text-emerald-200 dark:border-emerald-600";
      case "amber":
        return "bg-amber-100 text-amber-700 border-amber-600 dark:bg-amber-900 dark:text-amber-200 dark:border-amber-600";
      case "purple":
        return "bg-purple-100 text-purple-700 border-purple-600 dark:bg-purple-900 dark:text-purple-200 dark:border-purple-500 opacity-80";
      case "small-neutral":
        return "bg-neutral-50 text-neutral-600 border-neutral-300 dark:bg-neutral-900 dark:text-neutral-300 dark:border-neutral-500";
    }
  }

  $: sizeClasses = getSizeClasses(size);
  $: variantClasses = getVariantClasses(variant);
</script>

<span class="{baseClasses} {sizeClasses} {variantClasses}" {title}>
  <slot />
</span>
