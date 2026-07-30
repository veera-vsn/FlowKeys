import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { ClipboardPopup } from "./ClipboardPopup";
import { Toast } from "./Toast";

// All windows share one bundle; the label picks which screen to render.
const ROOTS: Record<string, () => React.JSX.Element> = {
  "clipboard-popup": ClipboardPopup,
  toast: Toast,
};

const label = getCurrentWindow().label;
// Every window shares one bundle, so all CSS lands in all of them. Tag the
// root element and scope window-specific html/body rules to it — otherwise a
// rule like `pointer-events: none` for the toast disables the whole app.
document.documentElement.dataset.window = label;

const Root = ROOTS[label] ?? App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
