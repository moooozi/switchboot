import { undoRedoStore } from "./stores/undoRedo";
import type { BootEntry, ChangeEvent } from "./types";
import type { FirmwareApi } from "./api";
import { OrderActions } from "./types";

export class OrderManager {
  private bootEntries: BootEntry[];
  private discoveredEntries: BootEntry[] = [];
  private efiSetupState = false;
  private dragStartOrder: number[] | null = null;

  constructor(
    initialEntries: BootEntry[],
    private onEntriesChanged?: (entries: BootEntry[]) => void,
    private onDiscoveredEntriesChanged?: (entries: BootEntry[]) => void,
    private onEfiSetupStateChanged?: (state: boolean) => void,
    private apiService?: FirmwareApi
  ) {
    this.bootEntries = [...initialEntries];
  }

  getBootEntries(): BootEntry[] {
    return [...this.bootEntries];
  }

  setBootEntries(entries: BootEntry[]): void {
    this.bootEntries = [...entries];
    this.onEntriesChanged?.(this.getBootEntries());
  }

  setDiscoveredEntries(entries: BootEntry[]): void {
    this.discoveredEntries = [...entries];
  }

  setFirmwareSetupState(state: boolean): void {
    this.efiSetupState = state;
  }

  private async applyBootNext(entryId: number | undefined): Promise<void> {
    if (!this.apiService) throw new Error("Firmware API is not initialized");
    if (entryId === undefined) await this.apiService.unsetBootNext();
    else await this.apiService.setBootNext(entryId);
    this.discoveredEntries = this.discoveredEntries.map((entry) => ({
      ...entry,
      is_bootnext: entry.id === entryId,
    }));
    this.onDiscoveredEntriesChanged?.([...this.discoveredEntries]);
  }

  private async changeBootNext(entryId: number | undefined, action: OrderActions): Promise<void> {
    await undoRedoStore.runExclusive(async () => {
      const previous = this.discoveredEntries.find((entry) => entry.is_bootnext)?.id;
      if (previous === entryId) return;
      await this.applyBootNext(entryId);
      undoRedoStore.addChange({
        action,
        description: action,
        undoCommand: () => this.applyBootNext(previous),
        redoCommand: () => this.applyBootNext(entryId),
      });
    });
  }

  setBootNext(entryId: number): Promise<void> {
    return this.changeBootNext(entryId, OrderActions.SetBootNext);
  }

  unsetBootNext(): Promise<void> {
    return this.changeBootNext(undefined, OrderActions.UnsetBootNext);
  }

  private async applyFirmwareSetup(state: boolean): Promise<void> {
    if (!this.apiService) throw new Error("Firmware API is not initialized");
    if (state) await this.apiService.setBootToFirmwareSetup();
    else await this.apiService.unsetBootToFirmwareSetup();
    this.efiSetupState = state;
    this.onEfiSetupStateChanged?.(state);
  }

  private async changeFirmwareSetup(state: boolean, action: OrderActions): Promise<void> {
    await undoRedoStore.runExclusive(async () => {
      const previous = this.efiSetupState;
      if (previous === state) return;
      await this.applyFirmwareSetup(state);
      undoRedoStore.addChange({
        action,
        description: action,
        undoCommand: () => this.applyFirmwareSetup(previous),
        redoCommand: () => this.applyFirmwareSetup(state),
      });
    });
  }

  setBootToFirmwareSetup(): Promise<void> {
    return this.changeFirmwareSetup(true, OrderActions.SetBootToFirmwareSetup);
  }

  unsetBootToFirmwareSetup(): Promise<void> {
    return this.changeFirmwareSetup(false, OrderActions.UnsetBootToFirmwareSetup);
  }

  private recordOrder(action: OrderActions, before: BootEntry[], after: BootEntry[]): ChangeEvent {
    const change: ChangeEvent = {
      action,
      description: action,
      undoCommand: () => this.setBootEntries(before),
      redoCommand: () => this.setBootEntries(after),
    };
    this.setBootEntries(after);
    undoRedoStore.addChange(change);
    return change;
  }

  changeOrder(action: OrderActions, originalOrder: number[], newOrder: number[]): ChangeEvent | undefined {
    if (undoRedoStore.isBusy) return;
    const available = [...this.bootEntries, ...this.discoveredEntries];
    const entriesFor = (order: number[]) => order.map((id) => {
      const entry = available.find((entry) => entry.id === id);
      if (!entry) throw new Error(`Boot entry ${id} is not available`);
      return entry;
    });
    return this.recordOrder(action, entriesFor(originalOrder), entriesFor(newOrder));
  }

  moveEntry(index: number, direction: "up" | "down"): void {
    if (undoRedoStore.isBusy || index < 0 || index >= this.bootEntries.length) return;
    const newIndex = direction === "up" ? index - 1 : index + 1;
    if (newIndex < 0 || newIndex >= this.bootEntries.length) return;
    const before = this.getBootEntries();
    const after = [...before];
    [after[index], after[newIndex]] = [after[newIndex], after[index]];
    this.recordOrder(direction === "up" ? OrderActions.MoveUp : OrderActions.MoveDown, before, after);
  }

  setEntriesFromDragDrop(entries: BootEntry[]): void {
    if (undoRedoStore.isBusy) return;
    this.recordOrder(OrderActions.ReorderByDrag, this.getBootEntries(), [...entries]);
  }

  startDrag(): void {
    if (undoRedoStore.isBusy) return;
    this.dragStartOrder = this.bootEntries.map((entry) => entry.id);
  }

  endDrag(entries: BootEntry[]): void {
    const original = this.dragStartOrder;
    this.dragStartOrder = null;
    if (!original || undoRedoStore.isBusy) return;
    const next = entries.map((entry) => entry.id);
    if (JSON.stringify(original) !== JSON.stringify(next)) {
      this.changeOrder(OrderActions.ReorderByDrag, original, next);
    }
  }

  makeDefault(entryId: number): void {
    if (undoRedoStore.isBusy) return;
    const index = this.bootEntries.findIndex((entry) => entry.id === entryId);
    if (index <= 0) return;
    const before = this.getBootEntries();
    this.recordOrder(OrderActions.MakeDefault, before, [before[index], ...before.filter((entry) => entry.id !== entryId)]);
  }

  addToBootOrder(entry: BootEntry, position?: number): void {
    if (undoRedoStore.isBusy || this.bootEntries.some((item) => item.id === entry.id)) return;
    const before = this.getBootEntries();
    const after = [...before];
    const index = position !== undefined && position >= 0 && position <= after.length ? position : after.length;
    after.splice(index, 0, entry);
    this.recordOrder(OrderActions.AddToBootOrder, before, after);
  }

  removeFromBootOrder(entryId: number): void {
    if (undoRedoStore.isBusy || !this.bootEntries.some((entry) => entry.id === entryId)) return;
    const before = this.getBootEntries();
    this.recordOrder(OrderActions.RemoveFromBootOrder, before, before.filter((entry) => entry.id !== entryId));
  }

  discardToOriginal(originalEntries: BootEntry[]): void {
    if (undoRedoStore.isBusy) return;
    this.recordOrder(OrderActions.DiscardChanges, this.getBootEntries(), [...originalEntries]);
  }
}
