// Entry of the detached log windows: only what the log pages need, so the
// window opens quickly; it is shown once its first frame is drawn.
import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";
import "./theme/appearance.css";
import "./theme/glassSurfaces.css";
import "./theme/selectionMenus.css";
import "./modules/nexus/nexus.css";
import { LogMonitorPage } from "./components/LogMonitorPage";
import { TextContextMenu } from "./components/TextContextMenu";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { I18nProvider } from "./i18n/I18nContext";
import { ThemeProvider } from "./theme/ThemeProvider";
import { FloatingScrollbars } from "./components/FloatingScrollbars";

document.documentElement.classList.add("log-monitor-window");
document.addEventListener("contextmenu", event => event.preventDefault(), { capture: true });
for (const type of ["dragover", "drop"] as const) window.addEventListener(type, event => event.preventDefault(), { capture: true, passive: false });

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <I18nProvider>
        <ThemeProvider>
          <LogMonitorPage />
          <TextContextMenu />
          <FloatingScrollbars />
        </ThemeProvider>
      </I18nProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
requestAnimationFrame(() => requestAnimationFrame(() => { void getCurrentWindow().show().catch(() => {}); }));
