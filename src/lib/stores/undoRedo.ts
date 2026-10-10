import { writable } from "svelte/store";
import type { ChangeEvent } from "../types";

export interface UndoRedoState {
  busy: boolean;
  canUndo: boolean;
  canRedo: boolean;
  undoDescription: string;
  redoDescription: string;
}

export class UndoRedoManager {
  private pending = false;

  get isBusy(): boolean {
    return this.pending;
  }

  async runExclusive(operation: () => void | Promise<void>): Promise<boolean> {
    if (this.pending) return false;
    this.pending = true;
    this.updateState();
    try {
      await operation();
      return true;
    } finally {
      this.pending = false;
      this.updateState();
    }
  }
  private history: ChangeEvent[] = [];
  private currentIndex: number = -1;
  private store = writable<UndoRedoState>({
    busy: false,
    canUndo: false,
    canRedo: false,
    undoDescription: "",
    redoDescription: "",
  });

  subscribe = this.store.subscribe;

  addChange(change: ChangeEvent): void {
    // Remove any changes after the current index (when user makes new changes after undoing)
    this.history = this.history.slice(0, this.currentIndex + 1);

    // Add the new change
    this.history.push(change);
    this.currentIndex++;

    // Limit history to 50 items to prevent memory issues
    if (this.history.length > 50) {
      this.history.shift();
      this.currentIndex--;
    }

    this.updateState();
  }

  async undo(): Promise<boolean> {
    if (!this.canUndo()) return false;
    return this.runExclusive(async () => {
      await this.history[this.currentIndex].undoCommand();
      this.currentIndex--;
    });
  }

  async redo(): Promise<boolean> {
    if (!this.canRedo()) return false;
    return this.runExclusive(async () => {
      await this.history[this.currentIndex + 1].redoCommand();
      this.currentIndex++;
    });
  }

  canUndo(): boolean {
    return !this.pending && this.currentIndex >= 0;
  }

  canRedo(): boolean {
    return !this.pending && this.currentIndex < this.history.length - 1;
  }

  private updateState(): void {
    const canUndo = this.canUndo();
    const canRedo = this.canRedo();

    this.store.set({
      busy: this.pending,
      canUndo,
      canRedo,
      undoDescription: canUndo
        ? this.history[this.currentIndex].description
        : "",
      redoDescription: canRedo
        ? this.history[this.currentIndex + 1].description
        : "",
    });
  }

  clear(): void {
    if (this.pending) throw new Error("Cannot clear history during an operation");
    this.history = [];
    this.currentIndex = -1;
    this.updateState();
  }
}

export const undoRedoStore = new UndoRedoManager();
