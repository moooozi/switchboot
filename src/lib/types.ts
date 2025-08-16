export type BootEntry = {
  id: number;
  description: string;
  is_default: boolean;
  is_bootnext: boolean;
  is_current: boolean;
};

export type ShortcutConfig = {
  name: string;
  entryId: number;
  reboot: boolean;
  iconId?: string;
};
