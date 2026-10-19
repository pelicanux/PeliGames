import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type ReactNode } from "react";
import type { GameInfo } from "./GameGrid";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { notifyError, notifyInfo } from "../services/notifications";
import { NexusPopup } from "../modules/nexus/NexusPopup";

interface ManualOptions { line: string; proton: string | null }

/** Pending edits replace Play until they are applied or discarded. */
export function SteamPendingActions({game, disabled, onSaved, children}: {
  game: GameInfo; disabled: boolean; onSaved: (game: GameInfo) => void; children: ReactNode;
}) {
  const pt = useI18n().language === "pt";
  const steamLabel = "Steam";
  const [busy, setBusy] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [pending, setPending] = useState(Boolean(game.steam_pending));
  const [manual, setManual] = useState<ManualOptions | null>(null);
  const [copied, setCopied] = useState(false);
  useEffect(() => { setPending(Boolean(game.steam_pending)); }, [game.external_id, game.steam_pending]);
  useEffect(() => { setManual(null); setCopied(false); }, [game.external_id]);
  const latest = useRef({game, onSaved, busy}); latest.current = {game, onSaved, busy};
  useEffect(() => {
    if (game.launcher !== "Steam" || !game.external_id) return;
    let live = true, inFlight = false, checkedAt = 0;
    let unlistenFocus: (() => void) | undefined;
    const refresh = () => {
      if (!live || inFlight || latest.current.busy || document.visibilityState === "hidden" || Date.now() - checkedAt < 750) return;
      inFlight = true; checkedAt = Date.now();
      const entry = latest.current.game;
      void invoke<Pick<GameInfo, "proton" | "steam_proton_name" | "steam_pending">>("steam_game_proton_status", {externalId: entry.external_id, prefix: entry.prefix || null}).then(next => {
        if (!live || latest.current.game.external_id !== entry.external_id) return;
        const current = latest.current.game;
        // A late focus response must not replace a choice just saved locally.
        if (current.proton !== entry.proton || current.advanced !== entry.advanced || Boolean(current.steam_pending) !== Boolean(entry.steam_pending)) return;
        setPending(Boolean(next.steam_pending));
        const proton = current.native_linux ? undefined : next.proton || undefined;
        const label = current.native_linux ? undefined : next.steam_proton_name || undefined;
        if ((current.proton || undefined) !== proton || (current.steam_proton_name || undefined) !== label || Boolean(current.steam_pending) !== Boolean(next.steam_pending)) {
          latest.current.onSaved({...current, proton, steam_proton_name: label, steam_pending: next.steam_pending});
        }
      }).catch(() => {}).finally(() => { inFlight = false; });
    };
    refresh();
    if (isTauri() && !("__PELI_UI_PREVIEW__" in window)) {
      void getCurrentWindow().onFocusChanged(event => { if (event.payload) refresh(); }).then(unlisten => {
        if (live) unlistenFocus = unlisten; else unlisten();
      }).catch(() => {});
    }
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    window.addEventListener("steamGameOptionsOpened", refresh);
    window.addEventListener("peligamesLibraryChanged", refresh);
    return () => {
      live = false;
      unlistenFocus?.();
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
      window.removeEventListener("steamGameOptionsOpened", refresh);
      window.removeEventListener("peligamesLibraryChanged", refresh);
    };
  }, [game.external_id, game.launcher]);
  const act = async (action: "restart" | "revert" | "confirm-manual") => {
    if (busy || disabled || exporting) return;
    setBusy(true);
    try {
      const next = await invoke<GameInfo>("steam_game_settings_action", {externalId: game.external_id, action});
      setPending(Boolean(next.steam_pending));
      onSaved({...game, ...next, cover_url: game.cover_url});
      window.dispatchEvent(new Event("peligamesLibraryChanged"));
      notifyInfo(steamLabel, action === "restart" ? (pt ? "Configurações aplicadas. Steam reiniciada." : "Settings applied. Steam restarted.") : action === "confirm-manual" ? (pt ? "Opções da Steam verificadas. Perfil preservado." : "Steam options verified. Profile preserved.") : (pt ? "Alterações revertidas." : "Changes reverted."));
      if (action !== "revert") setManual(null);
    } catch (error) { notifyError(steamLabel, String(error)); }
    finally { setBusy(false); }
  };
  const showManual = async () => {
    if (busy || disabled || exporting) return;
    setExporting(true);
    try {
      const result = await invoke<ManualOptions>("steam_game_launch_preview", {externalId: game.external_id});
      setManual(result); setCopied(false);
    } catch (error) { notifyError(steamLabel, String(error)); }
    finally { setExporting(false); }
  };
  const copy = async () => {
    if (!manual) return;
    try { await navigator.clipboard.writeText(manual.line); setCopied(true); }
    catch (error) { notifyError(steamLabel, String(error)); }
  };
  if (game.launcher !== "Steam" || !game.external_id) return children;
  const blocked = disabled || busy || exporting;
  const protonName = manual?.proton?.split(/[\\/]/).filter(Boolean).pop();
  return <>
    <div className="steam-action-slot">
      {pending ? <div className="steam-pending-actions">
        <p className="steam-pending-label" role="status"><strong>{pt ? "Conclua a configuração na Steam" : "Complete Steam setup"}</strong>{pt ? "Confira a linha fixa e o Proton nas propriedades do jogo." : "Check the fixed command and Proton in game properties."}</p>
        <div className="steam-pending-primary">
          <button type="button" className="btn btn-play modern-play" disabled={blocked} aria-busy={exporting} onClick={() => void showManual()}><MenuIcon name="settings" />{exporting ? (pt ? "Aguarde…" : "Please wait…") : (pt ? "Configurar na Steam" : "Configure in Steam")}</button>
          <button type="button" className="modern-mini-btn" disabled={blocked} onClick={() => void act("revert")}>{pt ? "Reverter alterações" : "Revert changes"}</button>
        </div>
        <button type="button" className="modern-mini-btn steam-manual-open" disabled={blocked} aria-busy={busy} onClick={() => void act("restart")}><MenuIcon name="repair" />{busy ? (pt ? "Aguarde…" : "Please wait…") : (pt ? "Reiniciar Steam e aplicar automaticamente" : "Restart Steam and apply automatically")}</button>
      </div> : children}
    </div>
    {manual && <NexusPopup className="steam-manual-popup" title={pt ? "Configurar na Steam" : "Configure in Steam"} titleIcon={<MenuIcon name="settings" />} onClose={() => { if (!busy) setManual(null); }} footer={<>
      <button type="button" className="modern-mini-btn" disabled={blocked || !pending} onClick={() => void act("confirm-manual")}>{pending ? (pt ? "Já adicionei · verificar" : "I added it · verify") : (pt ? "Sem alterações pendentes" : "No pending changes")}</button>
      <button type="button" className="btn btn-primary" onClick={() => void copy()}><MenuIcon name={copied ? "check" : "copy"} />{copied ? (pt ? "Copiado" : "Copied") : (pt ? "Copiar opções" : "Copy options")}</button>
    </>}>
      <p className="steam-manual-intro">{pt ? "Na Steam, abra Propriedades do jogo → Geral → Opções de inicialização e substitua a linha pelas opções abaixo. Acrescente seus argumentos adicionais, se houver." : "In Steam, open game Properties → General → Launch Options and replace the line with the options below. Append any additional game arguments you use."}</p>
      <label className="steam-manual-field"><span>{pt ? "Opções de inicialização geradas" : "Generated launch options"}</span><textarea readOnly spellCheck={false} rows={4} value={manual.line} onFocus={event => event.currentTarget.select()} /></label>
      {protonName && <div className="steam-manual-proton"><MenuIcon name="info" /><p>{pt ? <>Para usar <strong>{protonName}</strong>, abra Propriedades → Compatibilidade na Steam e selecione essa versão do Proton. Ela precisa estar disponível na sua instalação da Steam.</> : <>To use <strong>{protonName}</strong>, open Properties → Compatibility in Steam and select that Proton version. It must be available in your Steam installation.</>}</p></div>}
      <p className="steam-manual-help">{pt ? "Adicione esta linha uma vez e use “Já adicionei · verificar”. Depois, MangoHUD, geração de quadros e variáveis usarão o perfil atualizado na próxima abertura do jogo, sem reiniciar a Steam. A escolha do Proton continua em Compatibilidade." : "Add this command once and use “I added it · verify”. MangoHUD, frame generation and variables will then read the updated profile on the next game launch, without restarting Steam. Proton selection remains under Compatibility."}</p>
    </NexusPopup>}
  </>;
}
