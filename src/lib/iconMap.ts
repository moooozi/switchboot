/**
 * Maps EFI boot entry descriptions to icon identifiers.
 * Matches OS names with flexible patterns to handle variations.
 */
export function getIconId(description?: string): string {
  if (!description) return "";

  const d = description.toLowerCase();

  // Windows Boot Manager
  if (/windows boot manager/i.test(d)) return "windows";

  // macOS variants
  if (/mac\s?os|darwin|apple/i.test(d)) return "macos";

  // Ubuntu and derivatives (*buntu family)
  if (/ubuntu|kubuntu|xubuntu|lubuntu|edubuntu|ubuntustudio|buntu/i.test(d))
    return "ubuntu";

  // KDE Neon (Ubuntu-based but distinct)
  if (/neon|kde\s?neon/i.test(d)) return "neon";

  // Pop!_OS
  if (/pop!?_?os|pop[\s_-]?os/i.test(d)) return "popos";

  // Debian
  if (/debian/i.test(d)) return "debian";

  // Fedora
  if (/fedora/i.test(d)) return "fedora";

  // SUSE variants (openSUSE, SUSE Linux Enterprise)
  if (/suse|opensuse|sles/i.test(d)) return "suse";

  // CachyOS
  if (/cachyos/i.test(d)) return "cachyos";

  // Manjaro
  if (/manjaro/i.test(d)) return "manjaro";

  // Arch Linux
  if (/arch\s?linux|archlinux/i.test(d)) return "arch";

  // NixOS
  if (/nix\s?os|nixos/i.test(d)) return "nixos";

  // Deepin
  if (/deepin/i.test(d)) return "deepin";

  // Generic Linux fallback
  if (/linux|gnu/i.test(d)) return "linux";

  // Default fallback for unknown entries
  return "";
}
