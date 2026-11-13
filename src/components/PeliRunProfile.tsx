import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { GameInfo } from "./GameGrid";
import { normalizeAdvanced } from "../services/advancedSettings";
import { useI18n } from "../i18n/I18nContext";
import { notifyError } from "../services/notifications";
import { NexusPopup } from "../modules/nexus/NexusPopup";
import {PeliRunInstructions,type ProfileExport,type PeliRunTarget} from "./PeliRunInstructions";
import "./peliRunProfile.css";


/** Exports a wrapper; never writes another launcher's configuration or permissions. */
export function PeliRunProfile({ game, disabled, setupOnly = false, inline = false, onClose, actionLabel }: { game: GameInfo; disabled?: boolean; setupOnly?: boolean; inline?: boolean; onClose?: () => void; actionLabel?: string }) {
  const {t: tx} = useI18n();

  const [profile, setProfile] = useState<ProfileExport | null>(null);
  const [busy, setBusy] = useState(false);
  const target:PeliRunTarget=["Steam", "Heroic", "Lutris"].includes(game.launcher)?game.launcher as PeliRunTarget:"Outro";
  const [copied, setCopied] = useState("");
  const open = async () => {
    if (busy || disabled) return;
    setBusy(true);
    try {
      setProfile(await invoke<ProfileExport>("export_launch_profile", { name: game.name, path: game.path, externalId: game.external_id ?? null, advanced: normalizeAdvanced(game.advanced) }));
      setCopied("");
    } catch (error) { notifyError("PeliRun", String(error)); }
    finally { setBusy(false); }
  };
  useEffect(() => { if (setupOnly) void open(); }, [setupOnly]); // The saved game is fixed for this setup dialog.
  const copy = async (key: string, value: string) => {
    try { await navigator.clipboard.writeText(value); setCopied(key); }
    catch (error) { notifyError("PeliRun", String(error)); }
  };
  const details=profile&&<PeliRunInstructions profile={profile} initialTarget={target} copied={copied} onCopy={(key,value)=>void copy(key,value)} onRefresh={()=>void open()} disabled={Boolean(disabled||busy)}/>;
  return <section className={`pelirun-export${inline?" pelirun-export-inline":""}`}>
    {!setupOnly&&(!inline||!profile)&&<div className="pelirun-export-intro"><div><strong>PeliRun</strong><p>{tx("ui-peli-run-profile", "use_this_profile_in_another_launcher_settings_saved_here_apply_on_the_next_game_launch")}</p></div>
    <button type="button" className="modern-mini-btn" disabled={disabled || busy} onClick={() => void open()}>{busy ? (tx("ui-peli-run-profile", "preparing")) : (actionLabel ?? tx("ui-peli-run-profile", "use_in_another_launcher"))}</button></div>}
    {setupOnly && !profile && <div><p>{busy ? (tx("ui-peli-run-profile", "preparing_pelirun_profile")) : (tx("ui-peli-run-profile", "could_not_prepare_the_profile_try_again"))}</p><button type="button" className="modern-mini-btn" disabled={busy} onClick={() => void open()}>{tx("ui-peli-run-profile", "retry")}</button>{!inline && <button type="button" className="modern-mini-btn" onClick={onClose}>{tx("ui-peli-run-profile", "close")}</button>}</div>}
    {profile && (inline ? details : <NexusPopup title={`PeliRun · ${profile.id}`} onClose={() => { setProfile(null); onClose?.(); }}>{details}</NexusPopup>)}
  </section>;
}
