import { useState, type ReactNode } from "react";
import { openBrowserUrl } from "../../services/browser";
import { ModOptionSwitch } from "../../components/ModOptionRow";
import { ConfirmModal } from "../../components/ConfirmModal";
import { FeatureReportSection } from "../reports/FeatureReportSection";
import { featureReportContext } from "../reports/reportContext";
import { openDanielPreferences } from "./danielNr";
import { useDanielNr, type DanielPanelProps } from "./useDanielNr";
import { DanielRuntimeVersion } from "./DanielRuntimeVersion";
import { DanielLoadingDll } from "./DanielLoadingDll";
import { DanielGameGuide } from "./DanielGameGuide";
import "./danielNrTest.css";

function Step({ title, children }: { title: string; children: ReactNode }) {
  return <section className="daniel-nr-step"><header><h4>{title}</h4></header><div className="daniel-nr-step-content">{children}</div></section>;
}

export function DanielNrTestPanel(props: DanielPanelProps) {
  const { game, running = false } = props;
  const panel = useDanielNr(props);
  const { pt, state, request, blocked, loading, packageReady, selectedRuntime, unsupported, supportedProfile } = panel;
  const [confirmRemove, setConfirmRemove] = useState(false);
  return <FeatureReportSection label="Daniel NR (experimental)" context={featureReportContext(game, "daniel-nr", request.directory, state?.directory || "")}>
    <div className="modern-dlss-panels daniel-nr-workflow">
      <section className="modern-dlss-card daniel-nr-test" aria-label={pt ? "Instalação do Daniel NR" : "Daniel NR installation"}>
        <header className="daniel-nr-test-heading"><h3>Daniel NR</h3><span className="modern-meta-chip">Experimental</span></header>
        <p className="modern-dlss-description">{pt ? "Siga as etapas para preparar o ambiente, instalar neste jogo e ajustar a imagem durante a partida." : "Follow the steps to prepare the environment, install for this game and adjust the image while playing."}</p>
        {unsupported ? <p className="modern-dlss-description">{game.flatpak ? (pt ? "Launchers Flatpak ainda não são suportados neste teste." : "Flatpak launchers are not supported in this test yet.") : (pt ? "Selecione a versão Windows do jogo para usar Proton." : "Select the Windows game version to use Proton.")}</p> : <>
          {loading && <p role="status">{pt ? "Verificando instalação…" : "Checking installation…"}</p>}
          {!state && !loading && <div className="modern-dlss-actions"><button type="button" className="modern-mini-btn" disabled={blocked} onClick={() => void panel.chooseExecutable()}>{pt ? "Escolher executável" : "Choose executable"}</button><button type="button" className="modern-mini-btn" disabled={blocked} onClick={() => void panel.reload()}>{pt ? "Tentar novamente" : "Retry"}</button></div>}
          {state && <>
            {packageReady || state.installed ? <div className="daniel-nr-environment-link"><span>{pt ? "Ambiente preparado" : "Environment ready"}</span><button type="button" className="modern-mini-btn" disabled={blocked} onClick={openDanielPreferences}>{pt ? "Gerenciar complementos" : "Manage add-ons"}</button></div> : <Step title={pt ? "Prepare o ambiente" : "Prepare the environment"}>
              <p>{pt ? `Prepare o pacote ${selectedRuntime} em Configurações → Complementos → Daniel NR. Esse preparo é compartilhado entre os jogos.` : `Prepare package ${selectedRuntime} in Settings → Add-ons → Daniel NR. Preparation is shared across games.`}</p>
              <button type="button" className="modern-dlss-primary" disabled={blocked} onClick={openDanielPreferences}>{pt ? "Preparar ambiente" : "Prepare environment"}</button>
            </Step>}
            <Step title={state.installed ? (pt ? "Instalado neste jogo" : "Installed for this game") : (pt ? "Instale neste jogo" : "Install for this game")}>
              <p>{pt ? "Requer jogo Windows de 64 bits, DirectX 12, Radeon compatível e Proton baseado no Wine 11." : "Requires a 64-bit Windows game, DirectX 12, a compatible Radeon and Wine 11-based Proton."}</p>
              <details className="daniel-nr-destination"><summary>{pt ? "Executável e destino" : "Executable and destination"}</summary><p className="daniel-nr-path">{state.executable}</p><p>{pt ? "Escolha o executável que renderiza o jogo. Em jogos Unreal, geralmente fica em Binaries/Win64; o inicializador na raiz pode não carregar o mod na partida." : "Choose the executable that renders the game. Unreal games usually use Binaries/Win64; the launcher in the root folder may not load the mod during play."}</p>{!state.installed && <button type="button" className="modern-mini-btn" disabled={blocked} onClick={() => void panel.chooseExecutable()}>{pt ? "Alterar executável" : "Change executable"}</button>}</details>
              {state.conflict && <p className="daniel-nr-test-error" role="alert">{state.conflict}</p>}
              {!state.gpu_access && <p className="daniel-nr-test-error">{pt ? "Sem acesso à GPU via /dev/kfd. Confira as permissões da distribuição." : "No GPU access through /dev/kfd. Check your distribution’s permissions."}</p>}
              {!supportedProfile && <p>{pt ? "Registre este jogo na biblioteca principal para configurar a execução." : "Register this game in the main library to configure its launch."}</p>}
              {!state.installed && <>
                <label className="daniel-nr-confirm"><input type="checkbox" checked={panel.dx12} disabled={blocked || !packageReady} onChange={e => panel.setDx12(e.target.checked)}/>{pt ? "O jogo está configurado em DirectX 12" : "The game is configured for DirectX 12"}</label>
                <div className="modern-dlss-actions"><button type="button" className="modern-dlss-primary" disabled={blocked || !packageReady || state.pending_version_change || !panel.dx12 || Boolean(state.conflict) || !state.gpu_access || !supportedProfile || !state.loading_dll.options.some(o => o.available && (panel.proxy === "auto" || o.name === panel.proxy))} onClick={() => void panel.execute("install")}>{pt ? "Instalar e ativar" : "Install and enable"}</button></div>
              </>}
              {state.installed && !panel.linked && <button type="button" className="modern-mini-btn" disabled={blocked || !supportedProfile || state.pending_version_change} onClick={() => void panel.execute("link")}>{pt ? "Concluir vínculo de execução" : "Complete launch integration"}</button>}
            </Step>
            <Step title={pt ? "Abra o jogo e ajuste a imagem" : "Launch the game and adjust the image"}>
              <DanielGameGuide version={selectedRuntime}/>
            </Step>
          </>}
        </>}
        {running && <p role="status" className="modern-dlss-description daniel-nr-test-status">{pt ? "Jogo aberto. Feche-o para alterar a instalação ou as opções." : "Game running. Close it to change the installation or options."}</p>}
        {panel.progress && <p role="status" className="modern-dlss-description daniel-nr-test-status">{panel.progress}</p>}
        {panel.error && <p className="daniel-nr-test-error" role="alert">{panel.error}</p>}
      </section>
      {!unsupported && <section className="modern-dlss-card daniel-nr-test" aria-label={pt ? "Opções do Daniel NR" : "Daniel NR options"}>
        <h3>{pt ? "Opções deste jogo" : "Options for this game"}</h3>
        <p className="modern-dlss-description">{state?.installed ? (pt ? "As escolhas são aplicadas automaticamente e valem na próxima abertura. Ajustes de imagem ficam no painel dentro do jogo." : "Selections apply automatically on the next launch. Image settings remain in the in-game panel.") : (pt ? "Escolha a versão e a DLL antes de instalar. Depois da instalação, trocar a seleção aplica a mudança automaticamente." : "Choose the version and DLL before installing. After installation, selecting another option applies the change automatically.")}</p>
        {state && <>
          <DanielRuntimeVersion state={state} value={selectedRuntime} onChange={panel.selectRuntime} onRecover={() => void panel.execute("recover-version")} disabled={blocked}/>
          <DanielLoadingDll installed={state.installed} state={state.loading_dll} value={panel.proxy} onChange={panel.selectProxy} disabled={blocked || state.pending_version_change}/>
          <div className="modern-dlss-option"><div><span>{pt ? "Começar ligado" : "Start enabled"}</span><p>{pt ? "Inicia o Neural Rendering com o jogo. O vínculo do mod permanece instalado ao desligar." : "Starts Neural Rendering with the game. Disabling keeps the mod installed."}</p></div><ModOptionSwitch label={pt ? "Começar com Daniel NR ligado" : "Start with Daniel NR enabled"} enabled={state.installed && state.enabled} disabled={blocked || !state.installed || state.pending_version_change || !supportedProfile || (!state.enabled && Boolean(state.conflict))} onToggle={() => void panel.execute("toggle")}/></div>
          {state.installed && <div className="daniel-nr-uninstall"><div><span>{pt ? "Desinstalar deste jogo" : "Uninstall from this game"}</span><p>{pt ? "Restaura arquivos originais e preserva os ajustes em backup. Os componentes compartilhados continuam disponíveis." : "Restores original files and backs up settings. Shared components remain available."}</p></div><button type="button" className="modern-mini-btn" disabled={blocked || state.pending_version_change || !supportedProfile} onClick={() => setConfirmRemove(true)}>{pt ? "Desinstalar" : "Uninstall"}</button></div>}
        </>}
        <details className="daniel-nr-support"><summary>{pt ? "Diagnóstico e suporte" : "Diagnostics and support"}</summary>
          <p className="modern-dlss-description">{pt ? "Confira imagem e estabilidade. Um menu aberto não comprova processamento neural; os contadores dos logs ajudam a confirmar." : "Check picture and stability. An open menu does not prove neural processing; log counters help confirm it."}</p>
          <div className="modern-dlss-actions"><button type="button" className="modern-mini-btn" disabled={panel.busy || !state} onClick={panel.diagnose}>{pt ? "Ver diagnóstico" : "View diagnostics"}</button><button type="button" className="modern-mini-btn" disabled={panel.busy} onClick={() => void panel.reload()}>{pt ? "Verificar novamente" : "Check again"}</button><button type="button" className="modern-mini-btn" onClick={() => void openBrowserUrl("https://github.com/danielblnc/DLSS-NR-on-AMD").catch(panel.fail)}>{pt ? "Projeto do Daniel" : "Daniel’s project"}</button></div>
          {panel.diagnostics !== undefined && <pre className="daniel-nr-diagnostics" tabIndex={0}>{panel.diagnostics}</pre>}
        </details>
      </section>}
    </div>
    {confirmRemove && <ConfirmModal title={pt ? "Desinstalar Daniel NR deste jogo?" : "Uninstall Daniel NR from this game?"} message={pt ? "O vínculo de execução será removido e os arquivos originais serão restaurados. Ajustes editados serão preservados no backup do componente." : "The launch integration will be removed and original files restored. Edited settings will be preserved in the component’s backup."} confirmText={pt ? "Desinstalar" : "Uninstall"} cancelText={pt ? "Cancelar" : "Cancel"} onCancel={() => setConfirmRemove(false)} onConfirm={() => { setConfirmRemove(false); void panel.execute("remove"); }}/>}
  </FeatureReportSection>;
}
