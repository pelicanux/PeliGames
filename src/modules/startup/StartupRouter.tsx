import { lazy, Suspense, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ModalSurface } from "../../components/ModalSurface";
import { BootSplash, SplashDone } from "../../components/BootSplash";
// Loaded only when the launcher opens: Pelinstall never downloads the library,
// Matrix, Nexus or Settings code.
const App = lazy(() => import("../../App"));
// The installer window loads only the assistant.
const LaunchErrorWindow = lazy(() => import("../pelinstall/LaunchErrorWindow"));
const Pelinstall = lazy(() => import("../pelinstall/Pelinstall").then(module => ({ default: module.Pelinstall })));
export interface StartupInfo { module: string; executable?: string | null; entry?: string | null; error?: string | null; log?: string | null; }
export function StartupRouter() {
  const [startup, setStartup] = useState<StartupInfo | null>(null);
  const [error, setError] = useState("");
  const [reportOpen, setReportOpen] = useState(true);
  useEffect(() => {
    document.documentElement.classList.toggle("pelinstall-mode", startup?.module === "pelinstall" || startup?.module === "launch-error");
    return () => document.documentElement.classList.remove("pelinstall-mode");
  }, [startup?.module]);
  useEffect(() => { let active=true; invoke<StartupInfo>("get_startup_context").then(info => { if (active) setStartup(info); }).catch(error => { if(active) setError(String(error)); }); return () => {active=false;}; }, []);
  if (!startup) return error ? <><BootSplash message={error} /><SplashDone /></> : <BootSplash />;
  if (startup.module === "pelinstall") return <Suspense fallback={<BootSplash />}><Pelinstall startup={startup} /><SplashDone /></Suspense>;
  if (startup.module === "launch-error") return <Suspense fallback={null}><LaunchErrorWindow startup={startup} /><SplashDone /></Suspense>;
  // Shortcut that failed while the launcher was opened with it: reported over the library.
  const report = <ModalSurface className="modal-content dialog-glass pelinstall-surface" header={<h2>Falha ao executar</h2>} onDismiss={() => setReportOpen(false)}><p>O executável não foi iniciado corretamente. Abra o PeliGames para verificar a configuração.</p><p className="pelinstall-error" role="alert">{error || startup.error}</p>{startup.log && <p className="pelinstall-note">Log: {startup.log}</p>}</ModalSurface>;
  // The report waits for the launcher (and its styles) to load.
  return <Suspense fallback={<BootSplash />}><App initialGamePath={startup.entry || undefined} /><SplashDone />{startup.error && reportOpen && <div className="modal-overlay">{report}</div>}</Suspense>;
}
