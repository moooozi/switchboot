// Mock boot entries for development/testing

export type BootEntry = {
  id: number;
  description: string;
  is_bootnext: boolean;
  is_current: boolean;
};

export const mockBootEntries: BootEntry[] = [
  {
    id: 1,
    description: "Fedora 43",
    is_bootnext: false,
    is_current: false,
  },
  {
    id: 2,
    description: "Ubuntu 24.04",
    is_bootnext: true,
    is_current: false,
  },
  {
    id: 3,
    description: "Windows Boot Manager",
    is_bootnext: false,
    is_current: true,
  },
  {
    id: 5,
    description: "Arch Linux",
    is_bootnext: false,
    is_current: false,
  },
  {
    id: 7,
    description: "EFI: Network Boot",
    is_bootnext: false,
    is_current: false,
  },
  {
    id: 8,
    description: "EFI: USB Drive",
    is_bootnext: false,
    is_current: false,
  },
];
