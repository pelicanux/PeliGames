import React from "react";
import ReactDOM from "react-dom/client";
import { StartupRouter } from "./modules/startup/StartupRouter";
import { TextContextMenu } from "./components/TextContextMenu";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { I18nProvider } from "./i18n/I18nContext";
import { ThemeProvider } from "./theme/ThemeProvider";
import { FloatingScrollbars } from "./components/FloatingScrollbars";

// Suppress the webview menu, including dialogs rendered outside the root.
// Cover handlers still receive the event and open the application's own menu.
document.addEventListener("contextmenu", (event) => event.preventDefault(), { capture: true });

// A dropped browser URL must never replace the local launcher document.
// Cancel only the webview's default action; application drop handlers and
// Tauri's native file-drop events still receive their events normally.
for (const type of ["dragover", "drop"] as const) {
  window.addEventListener(type, event => event.preventDefault(), { capture: true, passive: false });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <I18nProvider>
        <ThemeProvider>
        <StartupRouter />
        <TextContextMenu />
        <FloatingScrollbars />
        </ThemeProvider>
      </I18nProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
