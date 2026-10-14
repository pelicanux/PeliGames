import React from "react";
import { LogMonitorPage } from "./components/LogMonitorPage";
import ReactDOM from "react-dom/client";
import "./App.css";
import "./theme/appearance.css";
import "./theme/glassSurfaces.css";
import "./theme/selectionMenus.css";
import "./modules/nexus/nexus.css";
const StartupRouter = React.lazy(() => import("./modules/startup/StartupRouter").then(module => ({ default: module.StartupRouter })));
import { TextContextMenu } from "./components/TextContextMenu";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { I18nProvider } from "./i18n/I18nContext";
import { ThemeProvider } from "./theme/ThemeProvider";
import { FloatingScrollbars } from "./components/FloatingScrollbars";
import { migrateCoverPaths } from "./services/customCovers";

if (["game", "nexus"].includes(new URLSearchParams(window.location.search).get("logWindow") || "")) {
  document.documentElement.classList.add("log-monitor-window");
}

// Suppress the webview menu, including dialogs rendered outside the root.
// Cover handlers still receive the event and open the application's own menu.
document.addEventListener("contextmenu", (event) => event.preventDefault(), { capture: true });

// A dropped browser URL must never replace the local launcher document.
// Cancel only the webview's default action; application drop handlers and
// Tauri's native file-drop events still receive their events normally.
for (const type of ["dragover", "drop"] as const) {
  window.addEventListener(type, event => event.preventDefault(), { capture: true, passive: false });
}

// Saved cover addresses are fixed before the library first reads them.
void migrateCoverPaths().finally(() => ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <I18nProvider>
        <ThemeProvider>
        {["game", "nexus"].includes(new URLSearchParams(window.location.search).get("logWindow") || "") ? <LogMonitorPage /> : <React.Suspense fallback={<p role="status">Carregando…</p>}><StartupRouter /></React.Suspense>}
        <TextContextMenu />
        <FloatingScrollbars />
        </ThemeProvider>
      </I18nProvider>
    </ErrorBoundary>
  </React.StrictMode>,
));
