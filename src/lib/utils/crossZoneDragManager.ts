import type { BootEntry } from "../types";

/**
 * Manages cross-zone drag and drop state for boot entries
 */
export class CrossZoneDragManager {
  private isDragging = false;
  private snapshotBootEntries: BootEntry[] = [];
  private snapshotOthers: BootEntry[] = [];

  /**
   * Start tracking a drag operation
   */
  startDrag(bootEntries: BootEntry[], others: BootEntry[]): void {
    if (this.isDragging) return;
    
    this.isDragging = true;
    this.snapshotBootEntries = [...bootEntries];
    this.snapshotOthers = [...others];
  }

  /**
   * End the drag operation
   */
  endDrag(): void {
    this.isDragging = false;
    this.snapshotBootEntries = [];
    this.snapshotOthers = [];
  }

  /**
   * Check if currently dragging
   */
  isActive(): boolean {
    return this.isDragging;
  }

  /**
   * Analyze finalized boot order changes and determine what actions to take
   */
  analyzeBootOrderChanges(finalItems: BootEntry[]): {
    addedFromOthers: Array<{ entry: BootEntry; position: number }>;
    removedToOthers: BootEntry[];
    isReorderOnly: boolean;
  } {
    const finalIds = finalItems.map(e => e.id);
    const originalBootIds = this.snapshotBootEntries.map(e => e.id);
    const originalOthersIds = this.snapshotOthers.map(e => e.id);

    // Items that moved from Others to Boot Order
    const addedFromOthers = finalItems
      .filter(e => originalOthersIds.includes(e.id) && !originalBootIds.includes(e.id))
      .map(entry => ({
        entry,
        position: finalItems.findIndex(e => e.id === entry.id)
      }));

    // Items that moved from Boot Order to Others
    const removedToOthers = this.snapshotBootEntries.filter(
      e => !finalIds.includes(e.id)
    );

    const isReorderOnly = addedFromOthers.length === 0 && removedToOthers.length === 0;

    return { addedFromOthers, removedToOthers, isReorderOnly };
  }

  /**
   * Analyze finalized others list changes
   */
  analyzeOthersChanges(finalItems: BootEntry[]): {
    addedFromBoot: BootEntry[];
  } {
    const finalIds = finalItems.map(e => e.id);
    const originalBootIds = this.snapshotBootEntries.map(e => e.id);
    const originalOthersIds = this.snapshotOthers.map(e => e.id);

    // Items that moved from Boot Order to Others
    const addedFromBoot = finalItems.filter(
      e => originalBootIds.includes(e.id) && !originalOthersIds.includes(e.id)
    );

    return { addedFromBoot };
  }
}
