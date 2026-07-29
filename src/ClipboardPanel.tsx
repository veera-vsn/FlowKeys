import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { timeAgo } from "./format";

interface ClipboardEntry {
  id: string;
  text: string;
  copied_at: number;
}

const UPDATED_EVENT = "clipboard://updated";
const SEARCH_DEBOUNCE_MS = 150;
const MAX_ENTRIES = 500;

function preview(text: string): string {
  const firstLine = text.split(/\r?\n/, 1)[0];
  return firstLine.length > 200 ? `${firstLine.slice(0, 200)}…` : firstLine;
}

export function ClipboardPanel() {
  const [entries, setEntries] = useState<ClipboardEntry[]>([]);
  const [results, setResults] = useState<ClipboardEntry[] | null>(null);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const searchToken = useRef(0);

  async function refresh() {
    const list = await invoke<ClipboardEntry[]>("list_clipboard_history");
    setEntries(list);
    const trimmed = query.trim();
    if (trimmed) {
      setResults(await invoke<ClipboardEntry[]>("list_clipboard_history", { query: trimmed }));
    }
  }

  useEffect(() => {
    let cancelled = false;
    invoke<ClipboardEntry[]>("list_clipboard_history").then((list) => {
      if (cancelled) return;
      setEntries(list);
      setLoading(false);
    });

    const unlisten = listen<ClipboardEntry>(UPDATED_EVENT, (event) => {
      setEntries((prev) =>
        [event.payload, ...prev.filter((e) => e.id !== event.payload.id)].slice(0, MAX_ENTRIES),
      );
    });

    return () => {
      cancelled = true;
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const trimmed = query.trim();
    if (!trimmed) {
      setResults(null);
      return;
    }
    const token = ++searchToken.current;
    const handle = setTimeout(async () => {
      const found = await invoke<ClipboardEntry[]>("list_clipboard_history", { query: trimmed });
      if (searchToken.current === token) {
        setResults(found);
      }
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(handle);
  }, [query]);

  const visible = results ?? entries;

  async function handleCopy(id: string) {
    setError(null);
    try {
      await invoke("copy_clipboard_entry", { id });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleRemove(id: string) {
    setError(null);
    try {
      await invoke("remove_clipboard_entry", { id });
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleClearAll() {
    if (!window.confirm("Clear all clipboard history? This can't be undone.")) return;
    setError(null);
    try {
      await invoke("clear_clipboard_history");
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  }

  const totalCount = entries.length;

  return (
    <div className="tab-panel">
      <div className="panel-header">
        <h2>Clipboard</h2>
        <button type="button" className="link-button danger" onClick={handleClearAll} disabled={totalCount === 0}>
          Clear all
        </button>
      </div>
      <p className="muted">
        Copy anything and it shows up here — up to 500 items, kept only on this device.
      </p>

      <input
        className="clipboard-search"
        type="search"
        placeholder="Search clipboard history…"
        value={query}
        onChange={(e) => setQuery(e.currentTarget.value)}
      />

      {error && <p className="field-error">{error}</p>}

      {loading ? (
        <p className="muted">Loading…</p>
      ) : visible.length === 0 ? (
        <p className="muted">
          {query.trim() ? "No matches." : "Nothing copied yet — copy some text to see it here."}
        </p>
      ) : (
        <ul className="clipboard-list">
          {visible.map((entry) => (
            <li key={entry.id} className="clipboard-row">
              <div className="clipboard-row-main">
                <span className="clipboard-text" title={entry.text}>
                  {preview(entry.text)}
                </span>
                <span className="muted clipboard-time">{timeAgo(entry.copied_at)}</span>
              </div>
              <div className="clipboard-row-actions">
                <button type="button" className="link-button" onClick={() => handleCopy(entry.id)}>
                  Copy
                </button>
                <button type="button" className="link-button danger" onClick={() => handleRemove(entry.id)}>
                  Remove
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
