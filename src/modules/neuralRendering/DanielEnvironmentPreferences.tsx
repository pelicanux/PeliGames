import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../../i18n/I18nContext";
import { ConfirmModal } from "../../components/ConfirmModal";
import { MenuIcon } from "../../components/MenuIcon";
import { SelectionMenu } from "../../components/SelectionMenu";
import { openDirectoryPicker, openFilePicker } from "../../services/tauriService";
import { DANIEL_COMPONENT_CHANGED, loadDanielComponent, operateDanielComponent, type DanielComponentState } from "./danielNr";
import "./danielEnvironment.css";

export function DanielEnvironmentPreferences({ disabled, onBusy }: { disabled: boolean; onBusy: (value: boolean) => void }) {
  const pt = useI18n().language === "pt";
  const [state, setState] = useState<DanielComponentState>();
  const [version, setVersion] = useState("0.6.0");
  const [removal, setRemoval] = useState<string | null>(null);
  const [compiler, setCompiler] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState(""), [progress, setProgress] = useState("");
  const mounted = useRef(true), operation = useRef(false);
  const blocked = disabled || busy || Boolean(removal);
  const selected = state?.runtimes.find(r => r.version === version);
  const ready = selected?.ready;
  const reload = async () => {
    try { const next = await loadDanielComponent(); if (mounted.current) setState(next); }
    catch (e) { if (mounted.current) setError(String(e)); }
  };
  useEffect(() => {
    mounted.current = true; void reload();
    const subscription = listen<{ directory: string; message: string }>("danielNrProgress", event => {
      if (mounted.current && operation.current && event.payload.directory === "") setProgress(event.payload.message);
    });
    void subscription.catch(() => {});
    return () => { mounted.current = false; void subscription.then(off => off()).catch(() => {}); };
  }, []);
  const run = async (removeVersion?: string) => {
    if (operation.current || (!removeVersion && blocked) || !state) return;
    operation.current = true; setBusy(true); onBusy(true); setError(""); setProgress("");
    try {
      const source = !removeVersion && version === "0.8.0" ? await openDirectoryPicker(pt ? "Pacote Daniel 0.8 preparado" : "Prepared Daniel 0.8 package") : undefined;
      if (!removeVersion && version === "0.8.0" && !source) return;
      const next = await operateDanielComponent(removeVersion ? "remove-package" : version === "0.8.0" ? "import-runtime" : "prepare", compiler, removeVersion || version, source || undefined);
      if (mounted.current) { setState(next); setProgress(removeVersion ? (pt ? `Pacote ${removeVersion} desinstalado. As instalações nos jogos foram preservadas.` : `Package ${removeVersion} uninstalled. Game installations were preserved.`) : (pt ? "Ambiente preparado. Volte ao jogo para instalar esta versão." : "Environment ready. Return to the game to install this version.")); }
      window.dispatchEvent(new Event(DANIEL_COMPONENT_CHANGED));
    } catch (e) { if (mounted.current) { setError(String(e)); setProgress(""); } }
    finally { operation.current = false; if (mounted.current) { setBusy(false); setRemoval(null); } onBusy(false); }
  };
  const requestRemoval = () => { if (!blocked) { setRemoval(version); onBusy(true); } };
  const closeRemoval = () => { if (!operation.current) { setRemoval(null); onBusy(false); } };
  const browseCompiler = async () => {
    try { const file = await openFilePicker("d3dcompiler_47.dll", ["dll"]); if (file && mounted.current) setCompiler(file); }
    catch (e) { if (mounted.current) setError(String(e)); }
  };
  return <section data-preference-id="runtime-daniel" className="mod-workspace daniel-component-workspace" aria-label={pt ? "Ambiente Daniel NR" : "Daniel NR environment"}>
    <header className="mod-intro"><h3>{pt ? "Preparar o Daniel NR" : "Prepare Daniel NR"}</h3><p>{pt ? "Prepare o componente neural aqui e escolha em quais jogos usar. Os pacotes são compartilhados entre os jogos." : "Prepare the neural component here, then choose which games use it. Packages are shared across games."}</p></header>
    <section className="mod-install-surface" aria-label={pt ? "Preparo do componente Daniel NR" : "Daniel NR component preparation"} aria-busy={busy}>
      <div className="mod-step-heading"><span className="mod-component-icon"><MenuIcon name="neural"/></span><div><h4>Daniel NR</h4><p>{pt ? "Neural Rendering · componente experimental" : "Neural Rendering · experimental component"}</p></div><span className="mod-status-badge" data-ready={Boolean(ready)} role="status">{!state ? (pt ? "Verificando…" : "Checking…") : ready ? (pt ? "Preparado" : "Ready") : (pt ? "Não preparado" : "Not prepared")}</span></div>
      {state && <>
        <div className="mod-setting-row"><div className="mod-row-copy"><strong>{pt ? "Versão do motor" : "Runtime version"}</strong><small>{pt ? "Cada versão tem seu pacote. A escolha para cada jogo é feita na biblioteca." : "Each version has its own package. Choose the version for each game in the library."}</small></div><SelectionMenu label={pt ? "Versão para preparar" : "Version to prepare"} value={version} options={state.runtimes.map(r => ({ value: r.version, label: r.version }))} onChange={value => { setVersion(value); setError(""); setProgress(""); }} disabled={blocked}/></div>
        {!ready && (version === "0.6.0" ? <>
          <div className="mod-setting-row daniel-compiler-row"><div className="mod-row-copy"><strong>{pt ? "Compilador de shaders" : "Shader compiler"}</strong><small>{pt ? "Selecione d3dcompiler_47.dll nativa de 64 bits de um jogo seu." : "Select a native 64-bit d3dcompiler_47.dll from a game you own."}</small></div><div className="daniel-compiler-controls"><input type="text" aria-label="d3dcompiler_47.dll" value={compiler} disabled={blocked} onChange={e => setCompiler(e.target.value)} placeholder={pt ? "Caminho da DLL nativa" : "Native DLL path"}/><button type="button" className="mod-action-secondary" disabled={blocked} onClick={() => void browseCompiler()}><MenuIcon name="folderSearch"/>{pt ? "Localizar" : "Browse"}</button></div></div>
          {!state.python_ready && <p className="mod-feedback-error" role="alert">{pt ? "Requer Python 3.12+. Nas versões 3.12/3.13, instale também zstandard da distribuição." : "Requires Python 3.12+. For 3.12/3.13, install your distribution’s zstandard module as well."}</p>}
        </> : <div className="mod-setting-row"><div className="mod-row-copy"><strong>{pt ? "Pacote experimental 0.8.0" : "Experimental 0.8.0 package"}</strong><small>{pt ? "Importe a pasta do pacote preparado. Ela já inclui d3dcompiler_47.dll; não é preciso selecioná-la novamente. O conteúdo é verificado antes da instalação." : "Import the prepared package folder. It already includes d3dcompiler_47.dll; no separate selection is needed. Contents are verified before installation."}</small></div></div>)}
        {!ready && <div className="mod-step-actions"><button type="button" className="mod-action-primary" disabled={blocked || (version === "0.6.0" && (!compiler || !state.python_ready))} onClick={() => void run()}><MenuIcon name={version === "0.8.0" ? "folder" : "download"}/>{busy ? (pt ? "Preparando…" : "Preparing…") : version === "0.8.0" ? (pt ? "Importar pacote 0.8.0" : "Import 0.8.0 package") : (pt ? "Preparar ambiente" : "Prepare environment")}</button></div>}
        {selected?.present && <div className="mod-setting-row"><div className="mod-row-copy"><strong>{pt ? `Pacote ${version}` : `Package ${version}`}</strong><small>{pt ? "Remove somente este pacote do launcher. Mantém a outra versão, os ajustes e as cópias já instaladas nos jogos. Para trocar versões em um jogo, prepare os dois pacotes." : "Removes only this launcher package. Keeps the other version, settings and copies already installed in games. To switch versions in a game, prepare both packages."}</small></div><button type="button" className="mod-action-secondary" disabled={blocked} onClick={requestRemoval}><MenuIcon name="trash"/>{pt ? `Desinstalar ${version}` : `Uninstall ${version}`}</button></div>}
      </>}
      {!state && error && <div className="mod-step-actions"><button className="mod-action-secondary" type="button" disabled={blocked} onClick={() => void reload()}><MenuIcon name="repair"/>{pt ? "Tentar novamente" : "Retry"}</button></div>}
      {progress && <p role="status" className="mod-inline-progress">{progress}</p>}
      {error && <p role="alert" className="mod-feedback-error">{error}</p>}
    </section>
    {removal && <ConfirmModal title={pt ? `Desinstalar pacote Daniel ${removal}?` : `Uninstall Daniel ${removal} package?`} message={pt ? `Será removido somente o pacote preparado ${removal}. A outra versão, as configurações e as instalações nos jogos serão mantidas. Será preciso preparar ou importar este pacote novamente para instalá-lo ou trocar versões em um jogo.` : `Only prepared package ${removal} will be removed. The other version, settings and game installations will be kept. Prepare or import this package again to install it or switch versions in a game.`} confirmText={pt ? "Desinstalar" : "Uninstall"} cancelText={pt ? "Cancelar" : "Cancel"} isDanger busy={busy} onCancel={closeRemoval} onConfirm={() => void run(removal)}/>}
    <section className="mod-next-step"><div className="mod-step-heading"><span className="mod-component-icon"><MenuIcon name="gamepad"/></span><div><h4>{pt ? "Instale nos jogos que você escolher" : "Install for the games you choose"}</h4><p>{pt ? "Biblioteca → selecione um jogo → Mods → Daniel NR (teste)." : "Library → select a game → Mods → Daniel NR (test)."}</p></div></div><p className="mod-step-note">{pt ? "Escolha a versão e a DLL de carregamento, depois instale para esse jogo. Preparar o ambiente não altera as instalações existentes." : "Choose the version and loading DLL, then install for that game. Preparing the environment does not change existing installations."}</p><div className="mod-requirement"><MenuIcon name="info"/><p>{version === "0.6.0" ? (pt ? "O preparo baixa dependências verificadas e um ROCm privado; reserve cerca de 3 GB. Os drivers do sistema não são alterados." : "Preparation downloads verified dependencies and a private ROCm runtime; allow about 3 GB. System drivers are not changed.") : (pt ? "A versão 0.8.0 requer um pacote experimental preparado. Depois de importar, selecione-a nos detalhes do jogo." : "Version 0.8.0 requires a prepared experimental package. After importing, select it in the game’s details.")}</p></div></section>
  </section>;
}
