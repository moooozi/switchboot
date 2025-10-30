import type { BootEntry, ChangeEvent } from './types';
import { OrderActions } from './types';
import { undoRedoStore } from './stores/undoRedo';

export class OrderManager {
  private bootEntries: BootEntry[] = [];
  private discoveredEntries: BootEntry[] = [];
  private onEntriesChanged?: (entries: BootEntry[]) => void;
  private onDiscoveredEntriesChanged?: (discoveredEntries: BootEntry[]) => void;
  private onEfiSetupStateChanged?: (state: boolean) => void;
  private apiService?: any; // We'll inject the API service
  private dragStartOrder: number[] | null = null;

  constructor(
    initialEntries: BootEntry[],
    onEntriesChanged?: (entries: BootEntry[]) => void,
    onDiscoveredEntriesChanged?: (discoveredEntries: BootEntry[]) => void,
    onEfiSetupStateChanged?: (state: boolean) => void,
    apiService?: any
  ) {
    this.bootEntries = [...initialEntries];
    this.onEntriesChanged = onEntriesChanged;
    this.onDiscoveredEntriesChanged = onDiscoveredEntriesChanged;
    this.onEfiSetupStateChanged = onEfiSetupStateChanged;
    this.apiService = apiService;
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

  /**
   * Set boot next for an entry
   */
  async setBootNext(entryId: number): Promise<void> {
    if (!this.apiService) return;

    // Store current bootnext state
    const currentBootNext = this.discoveredEntries.find(e => e.is_bootnext)?.id;

    // Perform the API call
    await this.apiService.setBootNext(entryId);

    // Update discovered entries
    const newDiscoveredEntries = this.discoveredEntries.map(e => ({
      ...e,
      is_bootnext: e.id === entryId
    }));
    this.discoveredEntries = newDiscoveredEntries;
    this.onDiscoveredEntriesChanged?.(newDiscoveredEntries);

    // Create undo event
    const undoCommand = async () => {
      if (currentBootNext !== undefined) {
        await this.apiService.setBootNext(currentBootNext);
        const undoEntries = this.discoveredEntries.map(e => ({
          ...e,
          is_bootnext: e.id === currentBootNext
        }));
        this.discoveredEntries = undoEntries;
        this.onDiscoveredEntriesChanged?.(undoEntries);
      } else {
        await this.apiService.unsetBootNext();
        const undoEntries = this.discoveredEntries.map(e => ({
          ...e,
          is_bootnext: false
        }));
        this.discoveredEntries = undoEntries;
        this.onDiscoveredEntriesChanged?.(undoEntries);
      }
    };

    const redoCommand = async () => {
      await this.apiService.setBootNext(entryId);
      const redoEntries = this.discoveredEntries.map(e => ({
        ...e,
        is_bootnext: e.id === entryId
      }));
      this.discoveredEntries = redoEntries;
      this.onDiscoveredEntriesChanged?.(redoEntries);
    };

    const changeEvent: ChangeEvent = {
      action: OrderActions.SetBootNext,
      undoCommand,
      redoCommand,
      description: OrderActions.SetBootNext
    };

    undoRedoStore.addChange(changeEvent);
  }

  /**
   * Unset boot next
   */
  async unsetBootNext(): Promise<void> {
    if (!this.apiService) return;

    // Store current bootnext state
    const currentBootNext = this.discoveredEntries.find(e => e.is_bootnext)?.id;

    // Perform the API call
    await this.apiService.unsetBootNext();

    // Update discovered entries
    const newDiscoveredEntries = this.discoveredEntries.map(e => ({
      ...e,
      is_bootnext: false
    }));
    this.discoveredEntries = newDiscoveredEntries;
    this.onDiscoveredEntriesChanged?.(newDiscoveredEntries);

    // Create undo event
    const undoCommand = async () => {
      if (currentBootNext !== undefined) {
        await this.apiService.setBootNext(currentBootNext);
        const undoEntries = this.discoveredEntries.map(e => ({
          ...e,
          is_bootnext: e.id === currentBootNext
        }));
        this.discoveredEntries = undoEntries;
        this.onDiscoveredEntriesChanged?.(undoEntries);
      }
    };

    const redoCommand = async () => {
      await this.apiService.unsetBootNext();
      const redoEntries = this.discoveredEntries.map(e => ({
        ...e,
        is_bootnext: false
      }));
      this.discoveredEntries = redoEntries;
      this.onDiscoveredEntriesChanged?.(redoEntries);
    };

    const changeEvent: ChangeEvent = {
      action: OrderActions.UnsetBootNext,
      undoCommand,
      redoCommand,
      description: OrderActions.UnsetBootNext
    };

    undoRedoStore.addChange(changeEvent);
  }

  /**
   * Centralized function to handle order changes and create undo/redo events
   */
  changeOrder(
    action: OrderActions,
    originalOrder: number[],
    newOrder: number[]
  ): ChangeEvent {
    const originalEntries = this.getBootEntries();

    // Create new order based on the provided order IDs
    const newEntries = newOrder.map(id =>
      originalEntries.find(entry => entry.id === id)!
    );

    // Undo command: restore to original order
    const undoCommand = () => {
      const undoEntries = originalOrder.map(id =>
        originalEntries.find(entry => entry.id === id)!
      );
      this.setBootEntries(undoEntries);
    };

    // Redo command: apply the new order
    const redoCommand = () => {
      this.setBootEntries(newEntries);
    };

    // Apply the change immediately
    this.setBootEntries(newEntries);

    // Create and register the change event
    const changeEvent: ChangeEvent = {
      action,
      undoCommand,
      redoCommand,
      description: action
    };

    undoRedoStore.addChange(changeEvent);

    return changeEvent;
  }

  /**
   * Move an entry up or down in the boot order
   */
  moveEntry(index: number, direction: 'up' | 'down'): void {
    if (direction === 'up' && index === 0) return;
    if (direction === 'down' && index === this.bootEntries.length - 1) return;

    const originalOrder = this.getBootEntries().map(e => e.id);
    const newIndex = direction === 'up' ? index - 1 : index + 1;

    // Swap entries
    const newEntries = [...this.getBootEntries()];
    [newEntries[index], newEntries[newIndex]] = [newEntries[newIndex], newEntries[index]];

    const newOrder = newEntries.map(e => e.id);
    const action = direction === 'up' ? OrderActions.MoveUp : OrderActions.MoveDown;

    this.changeOrder(action, originalOrder, newOrder);
  }

  /**
   * Set boot entries directly (used for drag and drop)
   */
  setEntriesFromDragDrop(entries: BootEntry[]): void {
    const originalOrder = this.getBootEntries().map(e => e.id);
    const newOrder = entries.map(e => e.id);

    this.changeOrder(OrderActions.ReorderByDrag, originalOrder, newOrder);
  }

  /**
   * Capture the original order when drag starts
   */
  startDrag(): void {
    this.dragStartOrder = this.getBootEntries().map(e => e.id);
  }

  /**
   * Apply drag and drop changes using the captured start order
   */
  endDrag(entries: BootEntry[]): void {
    if (!this.dragStartOrder) return;

    const newOrder = entries.map(e => e.id);

    // Only create a change event if the order actually changed
    if (JSON.stringify(this.dragStartOrder) !== JSON.stringify(newOrder)) {
      this.changeOrder(OrderActions.ReorderByDrag, this.dragStartOrder, newOrder);
    }

    this.dragStartOrder = null;
  }

  /**
   * Make an entry the default (move to first position)
   */
  makeDefault(entryId: number): void {
    const originalOrder = this.getBootEntries().map(e => e.id);
    const entryIndex = this.getBootEntries().findIndex(e => e.id === entryId);

    if (entryIndex === -1 || entryIndex === 0) return;

    // Create new order with selected entry first
    const newOrder = [
      entryId,
      ...originalOrder.slice(0, entryIndex),
      ...originalOrder.slice(entryIndex + 1)
    ];

    this.changeOrder(OrderActions.MakeDefault, originalOrder, newOrder);
  }

  /**
   * Add an entry to the boot order
   */
  addToBootOrder(entry: BootEntry): void {
    const originalOrder = this.getBootEntries().map(e => e.id);
    const newOrder = [...originalOrder, entry.id];

    this.changeOrder(OrderActions.AddToBootOrder, originalOrder, newOrder);
  }

  /**
   * Remove an entry from the boot order
   */
  removeFromBootOrder(entryId: number): void {
    const originalOrder = this.getBootEntries().map(e => e.id);
    const newOrder = originalOrder.filter(id => id !== entryId);

    this.changeOrder(OrderActions.RemoveFromBootOrder, originalOrder, newOrder);
  }

  /**
   * Set boot to firmware setup
   */
  async setBootToFirmwareSetup(): Promise<void> {
    if (!this.apiService) return;

    // Perform the API call
    await this.apiService.setBootToFirmwareSetup();
    this.onEfiSetupStateChanged?.(true);

    // Note: efiSetupState is managed separately in the main component
    // This action changes EFI state immediately and is undoable
    const undoCommand = async () => {
      await this.apiService.unsetBootToFirmwareSetup();
      this.onEfiSetupStateChanged?.(false);
    };

    const redoCommand = async () => {
      await this.apiService.setBootToFirmwareSetup();
      this.onEfiSetupStateChanged?.(true);
    };

    const changeEvent: ChangeEvent = {
      action: OrderActions.SetBootToFirmwareSetup,
      undoCommand,
      redoCommand,
      description: OrderActions.SetBootToFirmwareSetup
    };

    undoRedoStore.addChange(changeEvent);
  }

  /**
   * Unset boot to firmware setup
   */
  async unsetBootToFirmwareSetup(): Promise<void> {
    if (!this.apiService) return;

    // Perform the API call
    await this.apiService.unsetBootToFirmwareSetup();
    this.onEfiSetupStateChanged?.(false);

    const undoCommand = async () => {
      await this.apiService.setBootToFirmwareSetup();
      this.onEfiSetupStateChanged?.(true);
    };

    const redoCommand = async () => {
      await this.apiService.unsetBootToFirmwareSetup();
      this.onEfiSetupStateChanged?.(false);
    };

    const changeEvent: ChangeEvent = {
      action: OrderActions.UnsetBootToFirmwareSetup,
      undoCommand,
      redoCommand,
      description: OrderActions.UnsetBootToFirmwareSetup
    };

    undoRedoStore.addChange(changeEvent);
  }
}