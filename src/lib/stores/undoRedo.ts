import { writable } from "svelte/store";
import type { ChangeEvent } from "../types";

export interface UndoRedoState {
  canUndo: boolean;
  canRedo: boolean;
  undoDescription: string;
  redoDescription: string;
}

class UndoRedoManager {
  private history: ChangeEvent[] = [];
  private currentIndex: number = -1;
  private store = writable<UndoRedoState>({
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

  undo(): boolean {
    if (!this.canUndo()) return false;

    const change = this.history[this.currentIndex];
    change.undoCommand();
    this.currentIndex--;

    this.updateState();
    return true;
  }

  redo(): boolean {
    if (!this.canRedo()) return false;

    this.currentIndex++;
    const change = this.history[this.currentIndex];
    change.redoCommand();

    this.updateState();
    return true;
  }

  canUndo(): boolean {
    return this.currentIndex >= 0;
  }

  canRedo(): boolean {
    return this.currentIndex < this.history.length - 1;
  }

  private updateState(): void {
    const canUndo = this.canUndo();
    const canRedo = this.canRedo();

    this.store.set({
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
    this.history = [];
    this.currentIndex = -1;
    this.updateState();
  }
}

export const undoRedoStore = new UndoRedoManager();
