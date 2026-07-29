import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./Toast.css";

const SHOW_EVENT = "toast://show";

/// Renders the floating confirmation window. Showing, positioning, and
/// dismissing are all driven from Rust; this only paints the message.
export function Toast() {
  const [message, setMessage] = useState("");

  useEffect(() => {
    const unlisten = listen<string>(SHOW_EVENT, (event) => setMessage(event.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return <div className="toast-window">{message}</div>;
}
