import { computePosition, flip, offset, shift } from "@floating-ui/dom";
import { writable } from "svelte/store";

// Global context menu state
export interface ContextMenuState {
  show: boolean;
  x: number;
  y: number;
  owner?: string; // Identifier for which component opened the menu
  items: Array<{
    label: string;
    disabled?: boolean;
    title?: string;
    onclick: () => void;
  }>;
  onclose: () => void;
}

const initialState: ContextMenuState = {
  show: false,
  x: 0,
  y: 0,
  owner: undefined,
  items: [],
  onclose: () => {},
};

export const contextMenuStore = writable<ContextMenuState>(initialState);

// Helper function to calculate menu position using Floating UI
async function calculateMenuPosition(
  triggerElement: HTMLElement,
  items: ContextMenuState["items"],
  preferredPlacement:
    | "bottom-start"
    | "top-start"
    | "right-start"
    | "left-start" = "bottom-start"
): Promise<{ x: number; y: number }> {
  // Create a temporary menu element to measure dimensions
  const tempMenu = document.createElement("div");
  tempMenu.className =
    "fixed w-max flex flex-col bg-white dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-600 rounded-lg shadow-lg py-2 z-50 opacity-0 pointer-events-none";
  tempMenu.style.left = "-9999px";
  tempMenu.style.top = "-9999px";

  // Add menu items to measure
  items.forEach((item) => {
    const itemElement = document.createElement("button");
    itemElement.className =
      "w-auto text-left px-4 py-2 text-neutral-900 dark:text-neutral-100 transition-colors hover:bg-neutral-100 dark:hover:bg-neutral-700 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-transparent";
    itemElement.textContent = item.label;
    tempMenu.appendChild(itemElement);
  });

  document.body.appendChild(tempMenu);

  try {
    // Use Floating UI to calculate position
    const { x, y } = await computePosition(triggerElement, tempMenu, {
      placement: preferredPlacement,
      middleware: [
        offset(8), // 8px gap between trigger and menu
        flip(), // Flip to opposite side if no space
        shift({ padding: 10 }), // Keep within viewport with 10px padding
      ],
    });

    return { x, y };
  } finally {
    // Clean up temporary element
    document.body.removeChild(tempMenu);
  }
}

// Helper function to open a context menu (closes any existing ones)
export async function openContextMenu(options: {
  triggerElement: HTMLElement;
  items: ContextMenuState["items"];
  preferredPlacement?:
    | "bottom-start"
    | "top-start"
    | "right-start"
    | "left-start";
  onclose?: () => void;
  owner?: string;
}) {
  const {
    triggerElement,
    items,
    preferredPlacement = "bottom-start",
    onclose = () => {},
    owner,
  } = options;

  const { x, y } = await calculateMenuPosition(
    triggerElement,
    items,
    preferredPlacement
  );

  // Update store with calculated position
  contextMenuStore.set({
    show: true,
    x,
    y,
    owner,
    items,
    onclose,
  });
}

// Helper function to close the context menu
export function closeContextMenu() {
  contextMenuStore.update((state) => ({
    ...state,
    show: false,
    owner: undefined,
  }));
}

// Helper function to toggle a context menu for a specific owner
export async function toggleContextMenu(
  owner: string,
  options: {
    triggerElement: HTMLElement;
    items: ContextMenuState["items"];
    preferredPlacement?:
      | "bottom-start"
      | "top-start"
      | "right-start"
      | "left-start";
    onclose?: () => void;
  }
) {
  const {
    triggerElement,
    items,
    preferredPlacement = "bottom-start",
    onclose = () => {},
  } = options;

  // Get current state
  let currentState: ContextMenuState = {
    show: false,
    x: 0,
    y: 0,
    owner: undefined,
    items: [],
    onclose: () => {},
  };
  const unsubscribe = contextMenuStore.subscribe((state) => {
    currentState = state;
  });
  unsubscribe();

  // If menu is already open and owned by this component, close it
  if (currentState.show && currentState.owner === owner) {
    closeContextMenu();
    return;
  }

  const { x, y } = await calculateMenuPosition(
    triggerElement,
    items,
    preferredPlacement
  );

  // Update store with calculated position
  contextMenuStore.set({
    show: true,
    x,
    y,
    owner,
    items,
    onclose,
  });
}
