import {NativeTitleTooltips} from "./components/NativeTitleTooltips";
import "./theme/tooltips.css";
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
import { BootSplash, finishBootSplash } from "./components/BootSplash";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { I18nProvider } from "./i18n/I18nContext";
import { ThemeProvider } from "./theme/ThemeProvider";
import { FloatingScrollbars } from "./components/FloatingScrollbars";
import { migrateCoverPaths } from "./services/customCovers";
import { installFrameRateMeter } from "./services/frameRateMeter";

if (["game", "nexus"].includes(new URLSearchParams(window.location.search).get("logWindow") || "")) {
  document.documentElement.classList.add("log-monitor-window");
}

// WebKitGTK 2.54 loses parts of a transparent window, so the window may be
// opaque: square the frame so its corners do not show the window background.
void import("@tauri-apps/api/core").then(({ invoke }) => invoke<boolean>("window_opaque"))
  .then(opaque => { if (opaque) document.documentElement.dataset.opaqueWindow = "1"; })
  .catch(() => {});

// Restores the interface frame meter (Settings → Logs and more → Developer mode).
installFrameRateMeter();

// The window's outer boxes never scroll; WebKitGTK may still scroll them (a focus,
// an overflowing transform), which slides the title bar out of view. Put them back.
window.addEventListener("scroll", event => {
  const target = event.target === document ? document.scrollingElement : event.target as Element | null;
  if (!target || !(target === document.scrollingElement || target === document.body || target.id === "root" || target.classList.contains("app-wrapper"))) return;
  if (target.scrollTop || target.scrollLeft) { target.scrollTop = 0; target.scrollLeft = 0; }
}, { capture: true, passive: true });

// The start screen leaves once a module has drawn; if one fails to load, it still
// leaves so the error is visible.
window.setTimeout(() => { void finishBootSplash(); }, 8000);

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
        {["game", "nexus"].includes(new URLSearchParams(window.location.search).get("logWindow") || "") ? <LogMonitorPage /> : <React.Suspense fallback={<BootSplash />}><StartupRouter /></React.Suspense>}
        <NativeTitleTooltips />
        <TextContextMenu />
        <FloatingScrollbars />
        </ThemeProvider>
      </I18nProvider>
    </ErrorBoundary>
  </React.StrictMode>,
));
