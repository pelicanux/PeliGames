import {Trans} from "react-i18next";
import {engine} from "../i18n/engine";
import { listen } from "@tauri-apps/api/event";
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
  const {t: tx, language} = useI18n();

  const steamLabel = "Steam";
  const [busy, setBusy] = useState(false);
  const [restartPhase, setRestartPhase] = useState("");
  useEffect(() => {
    if (!isTauri() || !game.external_id) return;
    let live = true;
    let unlisten: (() => void) | undefined;
    void listen<{external_id: string; phase: string}>("steam-settings-progress", event => {
      if (live && event.payload.external_id === game.external_id) setRestartPhase(event.payload.phase);
    }).then(stop => { if (live) unlisten = stop; else stop(); }).catch(() => {});
    return () => { live = false; unlisten?.(); };
  }, [game.external_id]);
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
      void invoke<Pick<GameInfo, "proton" | "steam_proton_name" | "steam_pending" | "steam_profile_proton">>("steam_game_proton_status", {externalId: entry.external_id, prefix: entry.prefix || null}).then(next => {
        if (!live || latest.current.game.external_id !== entry.external_id) return;
        const current = latest.current.game;
        // A late focus response must not replace a choice just saved locally.
        if (current.proton !== entry.proton || current.steam_profile_proton !== entry.steam_profile_proton || current.advanced !== entry.advanced || Boolean(current.steam_pending) !== Boolean(entry.steam_pending)) return;
        setPending(Boolean(next.steam_pending));
        const proton = current.native_linux ? undefined : next.proton || undefined;
        const label = current.native_linux ? undefined : next.steam_proton_name || undefined;
        if ((current.steam_profile_proton || undefined) !== (next.steam_profile_proton || undefined) || (current.proton || undefined) !== proton || (current.steam_proton_name || undefined) !== label || Boolean(current.steam_pending) !== Boolean(next.steam_pending)) {
          latest.current.onSaved({...current, proton, steam_profile_proton: next.steam_profile_proton, steam_proton_name: label, steam_pending: next.steam_pending});
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
    setRestartPhase(action === "restart" ? "checking" : "");
    try {
      const next = await invoke<GameInfo>("steam_game_settings_action", {externalId: game.external_id, action});
      setPending(Boolean(next.steam_pending));
      onSaved({...game, ...next, cover_url: game.cover_url});
      window.dispatchEvent(new Event("peligamesLibraryChanged"));
      notifyInfo(steamLabel, action === "restart" ? (tx("ui-steam-pending-actions", "settings_applied_steam_restarted")) : action === "confirm-manual" ? (tx("ui-steam-pending-actions", "steam_options_verified_profile_preserved")) : (tx("ui-steam-pending-actions", "changes_reverted")));
      if (action !== "revert") setManual(null);
    } catch (error) {
      notifyError(steamLabel, String(error));
      // Applying may succeed even if reopening the client fails. Reflect the
      // actual pending state instead of asking the user to apply it twice.
      try {
        const next = await invoke<GameInfo>("steam_game_settings_action", {externalId: game.external_id, action: "status"});
        setPending(Boolean(next.steam_pending));
        onSaved({...game, ...next, cover_url: game.cover_url});
      } catch { /* Keep the last confirmed state if status is unavailable. */ }
    }
    finally { setBusy(false); setRestartPhase(""); }
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
  const phases: Record<string, string> = {checking: tx("ui-steam-pending-actions", "checking_steam"), closing: tx("ui-steam-pending-actions", "closing_steam"), terminating: tx("ui-steam-pending-actions", "terminating_the_unresponsive_client"), finishing: tx("ui-steam-pending-actions", "releasing_the_steam_client"), applying: tx("ui-steam-pending-actions", "applying_settings"), starting: tx("ui-steam-pending-actions", "starting_steam"), done: tx("ui-steam-pending-actions", "steam_started")};
  const protonName = manual?.proton?.split(/[\\/]/).filter(Boolean).pop();
  return <>
    <div className="steam-action-slot">
      {pending ? <div className="steam-pending-actions">
        <p className="steam-pending-label" role="status"><strong>{tx("ui-steam-pending-actions", "complete_steam_setup")}</strong>{tx("ui-steam-pending-actions", "link_the_profile_in_the_game_s_launch_options")}</p>
        <div className="steam-pending-primary">
          <button type="button" className="btn btn-play modern-play" disabled={blocked} aria-busy={exporting} onClick={() => void showManual()}><MenuIcon name="settings" />{exporting ? (tx("ui-steam-pending-actions", "please_wait")) : (tx("ui-steam-pending-actions", "configure_in_steam"))}</button>
          <button type="button" className="modern-mini-btn" disabled={blocked} onClick={() => void act("revert")}>{tx("ui-steam-pending-actions", "revert_changes")}</button>
        </div>
        <button type="button" className="modern-mini-btn steam-manual-open" disabled={blocked} aria-busy={busy} onClick={() => void act("restart")}><MenuIcon name="repair" />{busy ? (phases[restartPhase] || (tx("ui-steam-pending-actions", "please_wait"))) : (tx("ui-steam-pending-actions", "restart_steam_and_apply_automatically"))}</button>
      </div> : children}
    </div>
    {manual && <NexusPopup className="steam-manual-popup" title={tx("ui-steam-pending-actions", "configure_in_steam")} titleIcon={<MenuIcon name="settings" />} onClose={() => { if (!busy) setManual(null); }} footer={<>
      <button type="button" className="modern-mini-btn" disabled={blocked || !pending} onClick={() => void act("confirm-manual")}>{pending ? (tx("ui-steam-pending-actions", "i_added_it_verify")) : (tx("ui-steam-pending-actions", "no_pending_changes"))}</button>
      <button type="button" className="btn btn-primary" onClick={() => void copy()}><MenuIcon name={copied ? "check" : "copy"} />{copied ? (tx("ui-steam-pending-actions", "copied")) : (tx("ui-steam-pending-actions", "copy_options"))}</button>
    </>}>
      <p className="steam-manual-intro">{tx("ui-steam-pending-actions", "in_steam_open_game_properties_general_launch_options_and_replace_the_line_with_the_optio")}</p>
      <label className="steam-manual-field"><span>{tx("ui-steam-pending-actions", "generated_launch_options")}</span><textarea readOnly spellCheck={false} rows={4} value={manual.line} onFocus={event => event.currentTarget.select()} /></label>
      {protonName && <div className="steam-manual-proton"><MenuIcon name="info" /><p><Trans t={engine.getFixedT(language, "ui-steam-pending-actions")} i18nKey="select_proton_in_steam" values={{protonName}} components={{strong: <strong/>}}/></p></div>}
      <p className="steam-manual-help">{tx("ui-steam-pending-actions", "add_this_command_once_and_use_i_added_it_verify_mangohud_frame_generation_and_variables_")}</p>
    </NexusPopup>}
  </>;
}
