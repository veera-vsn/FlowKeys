export function formatShortcut(shortcut: string): string {
  return shortcut
    .split("+")
    .map((token) => {
      const lower = token.toLowerCase();
      if (lower === "control") return "Ctrl";
      if (lower === "alt") return "Alt";
      if (lower === "shift") return "Shift";
      if (lower === "super") return "Win";
      if (lower.startsWith("key")) return lower.slice(3).toUpperCase();
      if (lower.startsWith("digit")) return lower.slice(5);
      if (lower.startsWith("numpad")) return `Num ${lower.slice(6)}`;
      if (lower.startsWith("arrow")) return lower.slice(5);
      return lower.charAt(0).toUpperCase() + lower.slice(1);
    })
    .join(" + ");
}

export function timeAgo(atMs: number): string {
  const seconds = Math.max(0, Math.round((Date.now() - atMs) / 1000));
  if (seconds < 5) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  return `${days}d ago`;
}
