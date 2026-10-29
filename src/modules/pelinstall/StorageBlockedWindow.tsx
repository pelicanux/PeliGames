import "./installerStyles";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { MenuIcon } from "../../components/MenuIcon";
import { BrandName } from "../../components/BrandName";
import { useInstallerText } from "./installerText";
import { dragPelinstallWindow, PelinstallMinimizeButton } from "./WindowControls";
import type { StartupInfo } from "../startup/StartupRouter";

/** The launcher could not organize its files: explains why and retries, without loading the library. */
export default function StorageBlockedWindow({ startup }: { startup: StartupInfo }) {
  const tr = useInstallerText();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const preview = "__PELI_UI_PREVIEW__" in window;
  const close = () => { if (!preview) void getCurrentWindow().destroy().catch(reason => setError(String(reason))); };
  // Success restarts the launcher; only a refusal comes back here.
  const retry = async () => {
    if (busy || preview) return;
    setBusy(true); setError("");
    try { await invoke("retry_storage_organization"); } catch (reason) { setError(String(reason)); } finally { setBusy(false); }
  };
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
        <div className="pi-file-icon"><MenuIcon name="folder" /></div>
        <div className="pi-file-text"><strong>{tr("Organização dos arquivos")}</strong>{startup.log && <small title={startup.log}>{startup.log}</small>}</div>
      </div>
      <h2>{tr("Os arquivos ainda não foram reorganizados")}</h2>
      <p className="pi-hint">{tr("Esta versão guarda as configurações em pastas novas. Nada foi apagado; a cópia dos arquivos de antes está em migration-backup/layout-v2.")}</p>
      <p className="pi-hint">{tr("Feche versões antigas do PeliGames e os jogos abertos por ele, depois tente de novo.")}</p>
      {(error || startup.error) && <p className="pi-error" role="alert">{error || startup.error}</p>}
      <div className="pi-actions">
        <button type="button" className="pi-btn grow" onClick={close}>{tr("Fechar")}</button>
        <button type="button" className="pi-btn primary grow" disabled={busy} onClick={() => void retry()}>{busy ? tr("Aguarde…") : tr("Tentar de novo")}</button>
      </div>
    </div>
  </main>;
}
