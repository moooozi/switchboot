/**
 * Svelte action for smooth height transitions when content size changes.
 * Creates a "pillow" effect where the container maintains its size momentarily
 * before smoothly transitioning to the new height.
 */
export function smoothHeight(node: HTMLElement) {
  let prevHeight = 0;

  const resizeObserver = new ResizeObserver((entries) => {
    const newHeight = entries[0].contentRect.height;

    if (prevHeight > 0 && prevHeight !== newHeight) {
      node.style.height = `${prevHeight}px`;
      node.offsetHeight; // Force reflow

      requestAnimationFrame(() => {
        node.style.transition = "height 200ms ease-out";
        node.style.height = `${newHeight}px`;
      });
    }

    prevHeight = newHeight;
  });

  const contentNode = node.firstElementChild as HTMLElement;
  if (contentNode) resizeObserver.observe(contentNode);

  return { destroy: () => resizeObserver.disconnect() };
}
