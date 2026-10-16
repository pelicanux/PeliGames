import "./installerStyles";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { MenuIcon } from "../../components/MenuIcon";
import { BrandName } from "../../components/BrandName";
import { useInstallerText } from "./installerText";
import { dragPelinstallWindow, PelinstallMinimizeButton } from "./WindowControls";
import type { StartupInfo } from "../startup/StartupRouter";

/** A game started from its desktop shortcut failed: same look as the assistant. */
export default function LaunchErrorWindow({ startup }: { startup: StartupInfo }) {
  const tr = useInstallerText();
  const [error, setError] = useState("");
  const preview = "__PELI_UI_PREVIEW__" in window;
  const close = () => { if (!preview) void getCurrentWindow().destroy().catch(reason => setError(String(reason))); };
  const launch = async () => { try { await invoke("open_peligames_launcher", { path: startup.entry || null }); close(); } catch (reason) { setError(String(reason)); } };
  const entry = startup.entry || "";
  return <main className="pelinstall-window pi-window">
    <header className="pi-titlebar" onMouseDown={event => dragPelinstallWindow(event, setError)}>
      <h1 className="pi-brand"><img src="/peligames.svg" alt="" /><span className="pelinstall-brand"><BrandName /></span></h1>
      <span className="pi-spacer" />
      <PelinstallMinimizeButton onError={setError} />
      <button type="button" className="btn-titlebar window-control pi-close" aria-label={tr("Fechar")} title={tr("Fechar")} onClick={close}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" aria-hidden="true"><path d="M18 6 6 18M6 6l12 12" /></svg>
      </button>
    </header>
    <div className="pi-body">
      <div className="pi-file">
        <div className="pi-file-icon"><MenuIcon name="gamepad" /></div>
        <div className="pi-file-text"><strong title={entry}>{entry.split(/[\\/]/).pop() || tr("Executável")}</strong>{entry && <small title={entry}>{entry}</small>}</div>
        <span className="pi-chip" style={{ color: "var(--pi-danger)" }}>{tr("Falhou")}</span>
      </div>
      <h2>{tr("O jogo não abriu")}</h2>
      <p className="pi-hint">{tr("O executável não foi iniciado corretamente. Abra o PeliGames para verificar a configuração.")}</p>
      {(startup.error || error) && <p className="pi-error" role="alert">{error || startup.error}</p>}
      {startup.log && <div className="pi-notes"><p title={startup.log}>Log: {startup.log}</p></div>}
      <div className="pi-actions">
        <button type="button" className="pi-btn grow" onClick={close}>{tr("Fechar")}</button>
        <button type="button" className="pi-btn primary grow" onClick={() => void launch()}>{tr("Abrir no PeliGames")}</button>
      </div>
    </div>
  </main>;
}
