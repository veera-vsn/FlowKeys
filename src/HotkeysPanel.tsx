import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface HotkeyBinding {
  id: string;
  name: string;
  shortcut: string;
  enabled: boolean;
}

interface TriggerLogEntry {
  id: string;
  name: string;
  shortcut: string;
  at: number;
}

const TRIGGERED_EVENT = "hotkeys://triggered";
const MAX_VISIBLE_TRIGGERS = 8;

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

function formatShortcut(shortcut: string): string {
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

function eventToShortcut(e: React.KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(e.code)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Control");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  parts.push(e.code);
  return parts.join("+");
}

function timeAgo(atMs: number): string {
  const seconds = Math.max(0, Math.round((Date.now() - atMs) / 1000));
  if (seconds < 5) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  return `${hours}h ago`;
}

function ShortcutCapture({
  value,
  onChange,
}: {
  value: string;
  onChange: (shortcut: string) => void;
}) {
  const [capturing, setCapturing] = useState(false);
  const [heldMods, setHeldMods] = useState("");

  function handleKeyDown(e: React.KeyboardEvent) {
    e.preventDefault();
    if (e.code === "Escape") {
      setCapturing(false);
      setHeldMods("");
      return;
    }
    const shortcut = eventToShortcut(e);
    if (shortcut === null) {
      const mods: string[] = [];
      if (e.ctrlKey) mods.push("Ctrl");
      if (e.altKey) mods.push("Alt");
      if (e.shiftKey) mods.push("Shift");
      if (e.metaKey) mods.push("Win");
      setHeldMods(mods.join(" + "));
      return;
    }
    onChange(shortcut);
    setCapturing(false);
    setHeldMods("");
  }

  return (
    <input
      className="shortcut-capture"
      readOnly
      value={
        capturing
          ? heldMods
            ? `${heldMods} + …`
            : "Press a key combo…"
          : value
          ? formatShortcut(value)
          : "Click to set shortcut"
      }
      onFocus={() => setCapturing(true)}
      onBlur={() => {
        setCapturing(false);
        setHeldMods("");
      }}
      onKeyDown={handleKeyDown}
    />
  );
}

export function HotkeysPanel() {
  const [hotkeys, setHotkeys] = useState<HotkeyBinding[]>([]);
  const [triggers, setTriggers] = useState<TriggerLogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [newName, setNewName] = useState("");
  const [newShortcut, setNewShortcut] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [rowError, setRowError] = useState<Record<string, string>>({});

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      invoke<HotkeyBinding[]>("list_hotkeys"),
      invoke<TriggerLogEntry[]>("recent_triggers"),
    ]).then(([bindings, log]) => {
      if (cancelled) return;
      setHotkeys(bindings);
      setTriggers([...log].reverse().slice(0, MAX_VISIBLE_TRIGGERS));
      setLoading(false);
    });

    const unlisten = listen<TriggerLogEntry>(TRIGGERED_EVENT, (event) => {
      setTriggers((prev) => [event.payload, ...prev].slice(0, MAX_VISIBLE_TRIGGERS));
    });

    return () => {
      cancelled = true;
      unlisten.then((fn) => fn());
    };
  }, []);

  async function handleAdd(e: React.FormEvent) {
    e.preventDefault();
    setFormError(null);
    try {
      const binding = await invoke<HotkeyBinding>("add_hotkey", {
        name: newName,
        shortcut: newShortcut,
      });
      setHotkeys((prev) => [...prev, binding]);
      setNewName("");
      setNewShortcut("");
    } catch (err) {
      setFormError(String(err));
    }
  }

  async function handleToggle(binding: HotkeyBinding) {
    setRowError((prev) => ({ ...prev, [binding.id]: "" }));
    try {
      const updated = await invoke<HotkeyBinding>("update_hotkey", {
        id: binding.id,
        name: binding.name,
        shortcut: binding.shortcut,
        enabled: !binding.enabled,
      });
      setHotkeys((prev) => prev.map((h) => (h.id === updated.id ? updated : h)));
    } catch (err) {
      setRowError((prev) => ({ ...prev, [binding.id]: String(err) }));
    }
  }

  async function handleRemove(binding: HotkeyBinding) {
    try {
      await invoke("remove_hotkey", { id: binding.id });
      setHotkeys((prev) => prev.filter((h) => h.id !== binding.id));
    } catch (err) {
      setRowError((prev) => ({ ...prev, [binding.id]: String(err) }));
    }
  }

  return (
    <div className="tab-panel">
      <h2>Hotkeys</h2>
      <p className="muted">
        Global shortcuts work anywhere, even while FlowKeys is minimized to the tray.
      </p>

      {loading ? (
        <p className="muted">Loading…</p>
      ) : (
        <>
          <ul className="hotkey-list">
            {hotkeys.map((binding) => (
              <li key={binding.id} className="hotkey-row">
                <div className="hotkey-row-main">
                  <span className="hotkey-name">{binding.name}</span>
                  <kbd className="hotkey-combo">{formatShortcut(binding.shortcut)}</kbd>
                  <label className="toggle">
                    <input
                      type="checkbox"
                      checked={binding.enabled}
                      onChange={() => handleToggle(binding)}
                    />
                    <span>{binding.enabled ? "On" : "Off"}</span>
                  </label>
                  <button
                    type="button"
                    className="link-button danger"
                    onClick={() => handleRemove(binding)}
                  >
                    Remove
                  </button>
                </div>
                {rowError[binding.id] && <p className="field-error">{rowError[binding.id]}</p>}
              </li>
            ))}
            {hotkeys.length === 0 && <li className="muted">No hotkeys yet — add one below.</li>}
          </ul>

          <form className="hotkey-form" onSubmit={handleAdd}>
            <input
              className="hotkey-name-input"
              placeholder="Name (e.g. Toggle Clipboard)"
              value={newName}
              onChange={(e) => setNewName(e.currentTarget.value)}
              required
            />
            <ShortcutCapture value={newShortcut} onChange={setNewShortcut} />
            <button type="submit" disabled={!newName.trim() || !newShortcut}>
              Add hotkey
            </button>
          </form>
          {formError && <p className="field-error">{formError}</p>}

          <h3>Recently triggered</h3>
          {triggers.length === 0 ? (
            <p className="muted">Trigger a hotkey to see it appear here.</p>
          ) : (
            <ul className="trigger-log">
              {triggers.map((entry, i) => (
                <li key={`${entry.id}-${entry.at}-${i}`}>
                  <span>{entry.name}</span>
                  <span className="muted">{timeAgo(entry.at)}</span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </div>
  );
}
