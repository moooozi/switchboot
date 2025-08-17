export function getIconId(description?: string): string {
  if (!description) return "linux";
  const d = description.toLowerCase();
  if (/windows boot manager/i.test(d)) return "windows";
  if (/ubuntu/i.test(d)) return "ubuntu";
  if (/fedora/i.test(d)) return "fedora";
  if (/arch/i.test(d)) return "arch";
  if (/linux/i.test(d)) return "linux";
  return "";
}
