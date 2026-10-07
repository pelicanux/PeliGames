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
