import { useState } from "react";
import "./App.css";

type TabId = "general" | "hotkeys" | "clipboard" | "snippets" | "about";

interface Tab {
  id: TabId;
  label: string;
  status: "available" | "coming-soon";
}

const TABS: Tab[] = [
  { id: "general", label: "General", status: "available" },
  { id: "hotkeys", label: "Hotkeys", status: "coming-soon" },
  { id: "clipboard", label: "Clipboard", status: "coming-soon" },
  { id: "snippets", label: "Snippets", status: "coming-soon" },
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

function GeneralPanel() {
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
      <p>Version 0.1.0 &middot; Sprint 1: Foundation + Tray + Settings</p>
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
        {activeTab.id === "about" && <AboutPanel />}
        {activeTab.status === "coming-soon" && <TabPlaceholder label={activeTab.label} />}
      </main>
    </div>
  );
}

export default App;
