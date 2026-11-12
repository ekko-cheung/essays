import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { ConfirmProvider } from "./confirm";
import { ToastProvider } from "./toast";
import { initTheme } from "./theme";

initTheme();

// The webview's default right-click menu (Reload, Back, Inspect…) has no place in a
// release build. Keep it in dev, and keep Cut/Copy/Paste where text is editable.
if (!import.meta.env.DEV) {
  document.addEventListener("contextmenu", (e) => {
    const t = e.target as HTMLElement | null;
    if (!t?.closest("input, textarea, [contenteditable='true']")) e.preventDefault();
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ToastProvider>
      <ConfirmProvider>
        <App />
      </ConfirmProvider>
    </ToastProvider>
  </React.StrictMode>,
);

// The window starts hidden (see tauri.conf.json) so the user never sees an empty
// webview. Reveal it as soon as this module runs: requestAnimationFrame is
// suspended while the window is hidden, so gating on a paint callback left the
// reveal to the backend's fallback timer (seconds of nothing to look at).
// index.html has already parsed its themed splash by the time this runs.
void getCurrentWindow()
  .show()
  .catch(() => {
    /* not running inside Tauri (e.g. plain browser dev): nothing to show */
  });
