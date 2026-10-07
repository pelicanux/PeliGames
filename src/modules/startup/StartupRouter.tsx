import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "../../App";
import { ModalSurface } from "../../components/ModalSurface";
import { Pelinstall } from "../pelinstall/Pelinstall";
import { dragPelinstallWindow, PelinstallMinimizeButton } from "../pelinstall/WindowControls";
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
  if (!startup) return <div role="status">{error || "Carregando…"}</div>;
  if (startup.module === "pelinstall") return <Pelinstall startup={startup} />;
  const close = () => { if (!("__PELI_UI_PREVIEW__" in window)) void getCurrentWindow().destroy().catch(error => setError(String(error))); };
  const launch = async () => { try { await invoke("open_peligames_launcher", {path: startup.entry || null}); close(); } catch(error) {setError(String(error));} };
  const report = <ModalSurface className="modal-content dialog-glass pelinstall-surface" header={<h2>Falha ao executar</h2>} onHeaderMouseDown={startup.module === "launch-error" ? event => dragPelinstallWindow(event, setError) : undefined} headerActions={startup.module === "launch-error" ? <PelinstallMinimizeButton onError={setError} /> : undefined} onDismiss={startup.module === "launch-error" ? close : () => setReportOpen(false)}><p>O executável não foi iniciado corretamente. Abra o PeliGames para verificar a configuração.</p><p className="pelinstall-error" role="alert">{error || startup.error}</p>{startup.log && <p className="pelinstall-note">Log: {startup.log}</p>}{startup.module === "launch-error" && <footer className="pelinstall-actions"><button onClick={close}>Fechar</button><button className="primary" onClick={() => void launch()}>Abrir PeliGames</button></footer>}</ModalSurface>;
  if (startup.module === "launch-error") return <main className="pelinstall-window">{report}</main>;
  return <><App initialGamePath={startup.entry || undefined} />{startup.error && reportOpen && <div className="modal-overlay">{report}</div>}</>;
}
