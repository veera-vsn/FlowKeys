import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./ClipboardPopup.css";

interface ClipboardEntry {
  id: string;
  text: string;
  copied_at: number;
}

const SHOWN_EVENT = "clipboard-popup://shown";
const REFRESH_DEBOUNCE_MS = 100;

function preview(text: string): string {
  const firstLine = text.split(/\r?\n/, 1)[0];
  return firstLine.length > 140 ? `${firstLine.slice(0, 140)}…` : firstLine;
}

export function ClipboardPopup() {
  const [entries, setEntries] = useState<ClipboardEntry[]>([]);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  async function refresh(q: string) {
    const trimmed = q.trim();
    const list = await invoke<ClipboardEntry[]>(
      "list_clipboard_history",
      trimmed ? { query: trimmed } : {},
    );
    setEntries(list);
    setSelected(0);
  }

  async function resetAndFocus() {
    setQuery("");
    await refresh("");
    searchRef.current?.focus();
  }

  useEffect(() => {
    resetAndFocus();

    const unlistenShown = listen(SHOWN_EVENT, () => {
      resetAndFocus();
    });
    const unlistenFocus = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        invoke("hide_clipboard_popup");
      }
    });

    return () => {
      unlistenShown.then((fn) => fn());
      unlistenFocus.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const handle = setTimeout(() => refresh(query), REFRESH_DEBOUNCE_MS);
    return () => clearTimeout(handle);
  }, [query]);

  useEffect(() => {
    const item = listRef.current?.children[selected] as HTMLElement | undefined;
    item?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  function selectEntry(id: string) {
    invoke("select_clipboard_entry", { id });
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      invoke("hide_clipboard_popup");
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((i) => Math.min(i + 1, entries.length - 1));
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((i) => Math.max(i - 1, 0));
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      const entry = entries[selected];
      if (entry) selectEntry(entry.id);
    }
  }

  return (
    <div className="popup" onKeyDown={handleKeyDown}>
      <input
        ref={searchRef}
        className="popup-search"
        type="search"
        placeholder="Search clipboard…"
        value={query}
        onChange={(e) => setQuery(e.currentTarget.value)}
      />
      {entries.length === 0 ? (
        <p className="popup-empty">{query.trim() ? "No matches." : "Clipboard history is empty."}</p>
      ) : (
        <ul className="popup-list" ref={listRef}>
          {entries.map((entry, i) => (
            <li
              key={entry.id}
              className={i === selected ? "popup-item selected" : "popup-item"}
              onMouseEnter={() => setSelected(i)}
              onClick={() => selectEntry(entry.id)}
            >
              {preview(entry.text)}
            </li>
          ))}
        </ul>
      )}
      <p className="popup-hint">↑↓ to navigate &middot; Enter to copy &amp; paste &middot; Esc to close</p>
    </div>
  );
}
