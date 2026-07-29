import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { timeAgo } from "./format";

interface Snippet {
  id: string;
  name: string;
  trigger: string;
  content: string;
  enabled: boolean;
}

interface ExpandedEvent {
  name: string;
  at: number;
}

const EXPANDED_EVENT = "snippets://expanded";
const MAX_VISIBLE_EXPANSIONS = 8;

function preview(text: string): string {
  const firstLine = text.split(/\r?\n/, 1)[0];
  return firstLine.length > 80 ? `${firstLine.slice(0, 80)}…` : firstLine;
}

function emptyDraft(): { name: string; trigger: string; content: string } {
  return { name: "", trigger: "", content: "" };
}

export function SnippetsPanel() {
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [expansions, setExpansions] = useState<ExpandedEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [newSnippet, setNewSnippet] = useState(emptyDraft());
  const [formError, setFormError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editDraft, setEditDraft] = useState(emptyDraft());
  const [rowError, setRowError] = useState<Record<string, string>>({});

  useEffect(() => {
    let cancelled = false;
    invoke<Snippet[]>("list_snippets").then((list) => {
      if (cancelled) return;
      setSnippets(list);
      setLoading(false);
    });

    const unlisten = listen<ExpandedEvent>(EXPANDED_EVENT, (event) => {
      setExpansions((prev) => [event.payload, ...prev].slice(0, MAX_VISIBLE_EXPANSIONS));
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
      const snippet = await invoke<Snippet>("add_snippet", newSnippet);
      setSnippets((prev) => [...prev, snippet]);
      setNewSnippet(emptyDraft());
    } catch (err) {
      setFormError(String(err));
    }
  }

  function startEdit(snippet: Snippet) {
    setEditingId(snippet.id);
    setEditDraft({ name: snippet.name, trigger: snippet.trigger, content: snippet.content });
    setRowError((prev) => ({ ...prev, [snippet.id]: "" }));
  }

  function cancelEdit() {
    setEditingId(null);
    setEditDraft(emptyDraft());
  }

  async function handleSaveEdit(snippet: Snippet) {
    try {
      const updated = await invoke<Snippet>("update_snippet", {
        id: snippet.id,
        enabled: snippet.enabled,
        ...editDraft,
      });
      setSnippets((prev) => prev.map((s) => (s.id === updated.id ? updated : s)));
      setEditingId(null);
    } catch (err) {
      setRowError((prev) => ({ ...prev, [snippet.id]: String(err) }));
    }
  }

  async function handleToggle(snippet: Snippet) {
    setRowError((prev) => ({ ...prev, [snippet.id]: "" }));
    try {
      const updated = await invoke<Snippet>("update_snippet", {
        id: snippet.id,
        name: snippet.name,
        trigger: snippet.trigger,
        content: snippet.content,
        enabled: !snippet.enabled,
      });
      setSnippets((prev) => prev.map((s) => (s.id === updated.id ? updated : s)));
    } catch (err) {
      setRowError((prev) => ({ ...prev, [snippet.id]: String(err) }));
    }
  }

  async function handleRemove(snippet: Snippet) {
    try {
      await invoke("remove_snippet", { id: snippet.id });
      setSnippets((prev) => prev.filter((s) => s.id !== snippet.id));
    } catch (err) {
      setRowError((prev) => ({ ...prev, [snippet.id]: String(err) }));
    }
  }

  return (
    <div className="tab-panel">
      <h2>Snippets</h2>
      <p className="muted">
        Type a trigger anywhere on your system — a text field, a browser, another app — and it
        expands into the full text automatically. Pick triggers that wouldn't appear in normal
        typing, e.g. <kbd className="hotkey-combo">;addr</kbd> instead of <kbd className="hotkey-combo">addr</kbd>.
      </p>

      {loading ? (
        <p className="muted">Loading…</p>
      ) : (
        <>
          <ul className="hotkey-list">
            {snippets.map((snippet) =>
              editingId === snippet.id ? (
                <li key={snippet.id} className="hotkey-row">
                  <form
                    className="snippet-edit-form"
                    onSubmit={(e) => {
                      e.preventDefault();
                      handleSaveEdit(snippet);
                    }}
                  >
                    <input
                      className="hotkey-name-input"
                      placeholder="Name"
                      value={editDraft.name}
                      onChange={(e) => {
                        const name = e.currentTarget.value;
                        setEditDraft((d) => ({ ...d, name }));
                      }}
                      required
                    />
                    <input
                      className="snippet-trigger-input"
                      placeholder="Trigger (e.g. ;addr)"
                      value={editDraft.trigger}
                      onChange={(e) => {
                        const trigger = e.currentTarget.value;
                        setEditDraft((d) => ({ ...d, trigger }));
                      }}
                      required
                    />
                    <textarea
                      className="snippet-content-input"
                      placeholder="Expansion text"
                      value={editDraft.content}
                      onChange={(e) => {
                        const content = e.currentTarget.value;
                        setEditDraft((d) => ({ ...d, content }));
                      }}
                      required
                    />
                    <div className="snippet-edit-actions">
                      <button type="submit">Save</button>
                      <button type="button" className="link-button" onClick={cancelEdit}>
                        Cancel
                      </button>
                    </div>
                  </form>
                  {rowError[snippet.id] && <p className="field-error">{rowError[snippet.id]}</p>}
                </li>
              ) : (
                <li key={snippet.id} className="hotkey-row">
                  <div className="hotkey-row-main">
                    <span className="hotkey-name">
                      {snippet.name}
                      <span className="muted snippet-preview"> — {preview(snippet.content)}</span>
                    </span>
                    <kbd className="hotkey-combo">{snippet.trigger}</kbd>
                    <label className="toggle">
                      <input
                        type="checkbox"
                        checked={snippet.enabled}
                        onChange={() => handleToggle(snippet)}
                      />
                      <span>{snippet.enabled ? "On" : "Off"}</span>
                    </label>
                    <button type="button" className="link-button" onClick={() => startEdit(snippet)}>
                      Edit
                    </button>
                    <button
                      type="button"
                      className="link-button danger"
                      onClick={() => handleRemove(snippet)}
                    >
                      Remove
                    </button>
                  </div>
                  {rowError[snippet.id] && <p className="field-error">{rowError[snippet.id]}</p>}
                </li>
              ),
            )}
            {snippets.length === 0 && <li className="muted">No snippets yet — add one below.</li>}
          </ul>

          <form className="snippet-form" onSubmit={handleAdd}>
            <input
              className="hotkey-name-input"
              placeholder="Name (e.g. Home Address)"
              value={newSnippet.name}
              onChange={(e) => {
                const name = e.currentTarget.value;
                setNewSnippet((d) => ({ ...d, name }));
              }}
              required
            />
            <input
              className="snippet-trigger-input"
              placeholder="Trigger (e.g. ;addr)"
              value={newSnippet.trigger}
              onChange={(e) => {
                const trigger = e.currentTarget.value;
                setNewSnippet((d) => ({ ...d, trigger }));
              }}
              required
            />
            <textarea
              className="snippet-content-input"
              placeholder="Expansion text"
              value={newSnippet.content}
              onChange={(e) => {
                const content = e.currentTarget.value;
                setNewSnippet((d) => ({ ...d, content }));
              }}
              required
            />
            <button
              type="submit"
              disabled={!newSnippet.name.trim() || !newSnippet.trigger.trim() || !newSnippet.content.trim()}
            >
              Add snippet
            </button>
          </form>
          {formError && <p className="field-error">{formError}</p>}

          <h3>Recently expanded</h3>
          {expansions.length === 0 ? (
            <p className="muted">Type a trigger to see it appear here.</p>
          ) : (
            <ul className="trigger-log">
              {expansions.map((entry, i) => (
                <li key={`${entry.name}-${entry.at}-${i}`}>
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
