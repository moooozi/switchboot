// Mock boot entries for development/testing

export type BootEntry = {
  id: number;
  description: string;
  is_default: boolean;
  is_bootnext: boolean;
  is_current: boolean;
};

export const mockBootEntries: BootEntry[] = [
  {
    id: 1,
    description: "Windows Boot Manager",
    is_default: true,
    is_bootnext: false,
    is_current: false
  },
  {
    id: 2,
    description: "Ubuntu 24.04",
    is_default: false,
    is_bootnext: true,
    is_current: false
  },
  {
    id: 3,
    description: "Fedora 40",
    is_default: false,
    is_bootnext: false,
    is_current: true
  }
];