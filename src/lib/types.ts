export type BootEntry = {
  id: number;
  description: string;
  is_default: boolean | null;
  is_bootnext: boolean;
  is_current: boolean;
};

export type ShortcutConfig = {
  name: string;
  action: ShortcutAction;
  entryId?: number;
  reboot: boolean;
  iconId?: string;
};

export enum OrderActions {
  MoveUp = "Move Entry Up",
  MoveDown = "Move Entry Down",
  ReorderByDrag = "Move Entry",
  MakeDefault = "Make Default",
  AddToBootOrder = "Add to Boot Order",
  RemoveFromBootOrder = "Remove from Boot Order",
  SetBootNext = "Set Boot Next",
  UnsetBootNext = "Unset Boot Next",
  SetBootToFirmwareSetup = "Set Boot to Firmware Setup",
  UnsetBootToFirmwareSetup = "Unset Boot to Firmware Setup",
  DiscardChanges = "Discard Changes",
}

export interface ChangeEvent {
  action: OrderActions;
  undoCommand: () => void;
  redoCommand: () => void;
  description: string;
}

export enum ShortcutAction {
  SetBootNext = "SetBootNext",
  SetFirmwareSetup = "SetFirmwareSetup",
}