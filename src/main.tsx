import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { ConfirmProvider } from "./confirm";
import { initTheme } from "./theme";

initTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ConfirmProvider>
      <App />
    </ConfirmProvider>
  </React.StrictMode>,
);

// The window starts hidden (see tauri.conf.json) so the user never sees an empty
// webview. Reveal it once the themed splash/app has actually been painted.
requestAnimationFrame(() =>
  requestAnimationFrame(() => {
    try {
      void getCurrentWindow().show();
    } catch {
      /* not running inside Tauri (e.g. plain browser dev): nothing to show */
    }
  }),
);
