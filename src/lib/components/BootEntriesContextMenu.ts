import { openContextMenu } from "../stores/contextMenu";
import type { BootEntry } from "../types";

interface ContextMenuOptions {
  entry: BootEntry;
  mouseEvent: MouseEvent;
  busy: boolean;
  isPortable: boolean | null;
  onMakeDefault?: (entry: BootEntry) => void;
  onAddShortcut?: (entry: BootEntry) => void;
}

/**
 * Opens a context menu for a boot entry with appropriate actions
 */
export function openBootEntryContextMenu({
  entry,
  mouseEvent,
  busy,
  isPortable,
  onMakeDefault,
  onAddShortcut,
}: ContextMenuOptions) {
  mouseEvent.preventDefault();

  const isSpecialEntry = entry.id < 0;

  const trigger = document.createElement("div");
  Object.assign(trigger.style, {
    position: "fixed",
    left: `${mouseEvent.clientX}px`,
    top: `${mouseEvent.clientY}px`,
    width: "1px",
    height: "1px",
    opacity: "0",
    pointerEvents: "none",
  });
  document.body.appendChild(trigger);

  openContextMenu({
    triggerElement: trigger,
    items: [
      {
        label: "Make Default",
        disabled: entry.is_default || busy || isSpecialEntry,
        title: entry.is_default
          ? "Already default"
          : isSpecialEntry
            ? "Not possible for this entry"
            : "",
        onclick: () => onMakeDefault?.(entry),
      },
      {
        label: "Add Shortcut",
        disabled: busy || isPortable !== false,
        title:
          isPortable === true
            ? "Shortcuts are not available in portable mode"
            : isPortable === null
              ? "Loading portable mode status..."
              : "",
        onclick: () => onAddShortcut?.(entry),
      },
    ],
    preferredPlacement: "right-start",
    onclose: () =>
      document.body.contains(trigger) && document.body.removeChild(trigger),
    owner: "boot-entries-right-click",
  });
}
