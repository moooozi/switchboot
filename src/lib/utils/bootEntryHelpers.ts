import type { BootEntry } from "../types";

/** 
 * Entries cannot be dragged if:
 * - They have negative IDs (e.g., EFI Firmware Setup)
 * - They are the only item in boot order
 */
export const isDraggable = (entry: BootEntry, isOnlyEntry = false): boolean => 
  entry.id >= 0 && !isOnlyEntry;

/** Check if two arrays contain the same items (regardless of order) */
export const haveSameItems = (arr1: BootEntry[], arr2: BootEntry[]): boolean => {
  if (arr1.length !== arr2.length) return false;
  const ids1 = new Set(arr1.map((e) => e.id));
  const ids2 = new Set(arr2.map((e) => e.id));
  return ids1.size === ids2.size && [...ids1].every((id) => ids2.has(id));
};

/** Creates consistent dndzone configuration */
export const createDndConfig = (
  items: BootEntry[],
  disabled: boolean,
  flipDurationMs: number,
  zoneType: string
) => ({
  items,
  flipDurationMs,
  dropTargetStyle: {},
  dragDisabled: disabled,
  type: zoneType,
});
