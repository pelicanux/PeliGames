import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { GameInfo } from "./GameGrid";
import { normalizeAdvanced } from "../services/advancedSettings";
import { useI18n } from "../i18n/I18nContext";
import { notifyError } from "../services/notifications";
import { NexusPopup } from "../modules/nexus/NexusPopup";
import { MenuIcon } from "./MenuIcon";

interface ProfileExport { id: string; executable: string; arguments: string; line: string }
type Target = "Steam" | "Heroic" | "Lutris" | "Outro";

/** Exports a wrapper; never writes another launcher's configuration or permissions. */
export function PeliRunProfile({ game, disabled, setupOnly = false, inline = false, onClose }: { game: GameInfo; disabled?: boolean; setupOnly?: boolean; inline?: boolean; onClose?: () => void }) {
  const {t: tx} = useI18n();

  const [profile, setProfile] = useState<ProfileExport | null>(null);
  const [busy, setBusy] = useState(false);
  const [target, setTarget] = useState<Target>(["Steam", "Heroic", "Lutris"].includes(game.launcher) ? game.launcher as Target : "Outro");
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
  const field = (label: string, key: string, value: string) => <label className="pelirun-export-field"><span>{label}</span><div><input aria-label={label} readOnly value={value} onFocus={event => event.target.select()} /><button type="button" className="modern-mini-btn" onClick={() => void copy(key, value)}><MenuIcon name="copy" />{copied === key ? (tx("ui-peli-run-profile", "copied")) : (tx("ui-peli-run-profile", "copy"))}</button></div></label>;
  const details = profile && <>
      <p>{tx("ui-peli-run-profile", "select_where_you_will_launch_this_game_the_identifier_and_profile_stay_the_same_across_l")}</p>
      <div className="pelirun-export-targets" role="group" aria-label={tx("ui-peli-run-profile", "target_launcher")}>{(["Steam", "Heroic", "Lutris", "Outro"] as Target[]).map(item => <button key={item} type="button" className="modern-mini-btn" aria-pressed={target === item} onClick={() => { setTarget(item); setCopied(""); }}>{item === "Outro" ? tx("ui-peli-run-profile", "other_launcher") : item}</button>)}</div>
      <p>{target === "Steam" ? (tx("ui-peli-run-profile", "game_properties_general_launch_options_use_the_line_below_and_keep_proton_selection_in_s")) : target === "Heroic" ? (tx("ui-peli-run-profile", "game_settings_advanced_wrapper_command_enter_the_executable_and_arguments_in_their_respe")) : target === "Lutris" ? (tx("ui-peli-run-profile", "configure_game_system_options_command_prefix_use_the_line_below")) : (tx("ui-peli-run-profile", "use_the_launcher_s_wrapper_or_command_prefix_setting_it_must_pass_the_complete_game_comm"))}</p>
      {target === "Heroic" ? <>{field(tx("ui-peli-run-profile", "executable"), "binary", profile.executable)}{field(tx("ui-peli-run-profile", "wrapper_arguments"), "args", profile.arguments)}</> : field(tx("ui-peli-run-profile", "command"), "line", `${profile.line}${target === "Steam" ? " %command%" : ""}`)}
      <p className="pelirun-export-note">{tx("ui-peli-run-profile", "set_up_once_then_save_changes_in_peligames_and_relaunch_the_game_this_does_not_automatic")}</p>
      <p className="pelirun-export-note">{tx("ui-peli-run-profile", "flatpak_the_launcher_needs_access_to_pelirun_its_profile_and_the_game_s_components_no_pe")}</p>
  </>;
  return <section className="pelirun-export" style={inline ? { display: "block" } : undefined}>
    {!setupOnly && <><div><strong>PeliRun</strong><p>{tx("ui-peli-run-profile", "use_this_profile_in_another_launcher_settings_saved_here_apply_on_the_next_game_launch")}</p></div>
    <button type="button" className="modern-mini-btn" disabled={disabled || busy} onClick={() => void open()}>{busy ? (tx("ui-peli-run-profile", "preparing")) : (tx("ui-peli-run-profile", "use_in_another_launcher"))}</button></>}
    {setupOnly && !profile && <div><p>{busy ? (tx("ui-peli-run-profile", "preparing_pelirun_profile")) : (tx("ui-peli-run-profile", "could_not_prepare_the_profile_try_again"))}</p><button type="button" className="modern-mini-btn" disabled={busy} onClick={() => void open()}>{tx("ui-peli-run-profile", "retry")}</button>{!inline && <button type="button" className="modern-mini-btn" onClick={onClose}>{tx("ui-peli-run-profile", "close")}</button>}</div>}
    {profile && (inline ? details : <NexusPopup title={`PeliRun · ${profile.id}`} onClose={() => { setProfile(null); onClose?.(); }}>{details}</NexusPopup>)}
  </section>;
}
