import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HotkeysPanel } from "./HotkeysPanel";
import { ClipboardPanel } from "./ClipboardPanel";
import { SnippetsPanel } from "./SnippetsPanel";
import "./App.css";

type TabId = "general" | "hotkeys" | "clipboard" | "snippets" | "about";

interface Tab {
  id: TabId;
  label: string;
  status: "available" | "coming-soon";
}

const TABS: Tab[] = [
  { id: "general", label: "General", status: "available" },
  { id: "hotkeys", label: "Hotkeys", status: "available" },
  { id: "clipboard", label: "Clipboard", status: "available" },
  { id: "snippets", label: "Snippets", status: "available" },
  { id: "about", label: "About", status: "available" },
];

function TabPlaceholder({ label }: { label: string }) {
  return (
    <div className="tab-panel placeholder">
      <p>{label} isn't wired up yet.</p>
      <p className="muted">This shell ships first; {label.toLowerCase()} lands in a later sprint.</p>
    </div>
  );
}

interface Settings {
  copy_on_selection: boolean;
}

function GeneralPanel() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<Settings>("get_settings").then((loaded) => {
      if (!cancelled) setSettings(loaded);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  async function toggleCopyOnSelection(enabled: boolean) {
    setError(null);
    try {
      setSettings(await invoke<Settings>("set_copy_on_selection", { enabled }));
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div className="tab-panel">
      <h2>General</h2>
      <label className="setting-row">
        <span>Launch FlowKeys at login</span>
        <input type="checkbox" disabled />
      </label>
      <label className="setting-row">
        <span>Keep running in the background when the window is closed</span>
        <input type="checkbox" checked disabled />
      </label>
      <label className="setting-row">
        <span>Copy text as soon as I select it</span>
        <input
          type="checkbox"
          checked={settings?.copy_on_selection ?? false}
          disabled={settings === null}
          onChange={(e) => toggleCopyOnSelection(e.currentTarget.checked)}
        />
      </label>
      <p className="muted setting-note">
        Selecting text with the mouse copies it automatically, anywhere on your system. FlowKeys
        does this by sending <kbd className="hotkey-combo">Ctrl+C</kbd>, so leave it off if you
        select text in terminals — there <kbd className="hotkey-combo">Ctrl+C</kbd> interrupts
        whatever is running.
      </p>
      {error && <p className="field-error">{error}</p>}
      <p className="muted">
        Closing this window minimizes FlowKeys to the system tray — use the tray icon to reopen
        Settings or quit.
      </p>
    </div>
  );
}

function AboutPanel() {
  return (
    <div className="tab-panel">
      <h2>About FlowKeys</h2>
      <p>Version 0.1.0 &middot; Sprint 5: Snippets (text expansion)</p>
      <p className="muted">
        One fast, native-feeling, offline-first utility for hotkeys, clipboard history, and text
        snippets — no account required.
      </p>
    </div>
  );
}

function App() {
  const [active, setActive] = useState<TabId>("general");
  const activeTab = TABS.find((tab) => tab.id === active)!;

  return (
    <div className="app-shell">
      <nav className="sidebar">
        <div className="brand">FlowKeys</div>
        <ul>
          {TABS.map((tab) => (
            <li key={tab.id}>
              <button
                className={tab.id === active ? "nav-item active" : "nav-item"}
                onClick={() => setActive(tab.id)}
              >
                {tab.label}
                {tab.status === "coming-soon" && <span className="badge">soon</span>}
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <main className="content">
        {activeTab.id === "general" && <GeneralPanel />}
        {activeTab.id === "hotkeys" && <HotkeysPanel />}
        {activeTab.id === "clipboard" && <ClipboardPanel />}
        {activeTab.id === "snippets" && <SnippetsPanel />}
        {activeTab.id === "about" && <AboutPanel />}
        {activeTab.status === "coming-soon" && <TabPlaceholder label={activeTab.label} />}
      </main>
    </div>
  );
}

export default App;
