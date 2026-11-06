import {PeliRunProfile} from "../../components/PeliRunProfile";
import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {openUrl} from "@tauri-apps/plugin-opener";
import type {GameInfo} from "../../components/GameGrid";
import {MenuIcon} from "../../components/MenuIcon";
import {NexusPopup} from "../nexus/NexusPopup";
import {useI18n} from "../../i18n/I18nContext";

export interface MakoSandboxStatus {app_id:string;runtime:string;branch:string|null;extension_installed:boolean;ready:boolean;message:string;commands:string[];release_url:string}
export function MakoLaunchSetup({game,enabled=true,onClose,onSaved,sandbox,onCheckSandbox}:{game:GameInfo;enabled?:boolean;onClose:()=>void;onSaved:(game:GameInfo)=>void;sandbox?:MakoSandboxStatus;onCheckSandbox?:()=>Promise<void>}) {
  const {t: tx} = useI18n();

 const [line,setLine]=useState("");const [error,setError]=useState("");const [busy,setBusy]=useState(false);const [copied,setCopied]=useState(false);
 useEffect(()=>{let live=true;if(!sandbox&&game.launcher==="Steam"&&game.external_id)void invoke<{line:string}>("steam_game_launch_preview",{externalId:game.external_id}).then(r=>{if(live)setLine(r.line)}).catch(e=>{if(live)setError(String(e))});return()=>{live=false}},[game.external_id,sandbox]);
 const copy=async()=>{try{await navigator.clipboard.writeText(sandbox?sandbox.commands.join("\n"):line);setCopied(true)}catch(e){setError(String(e))}};
 const apply=async(action:"restart"|"confirm-manual")=>{setBusy(true);setError("");try{const updated=await invoke<GameInfo>("steam_game_settings_action",{externalId:game.external_id,action});onSaved({...game,...updated});window.dispatchEvent(new Event("peligamesLibraryChanged"));onClose()}catch(e){setError(String(e))}finally{setBusy(false)}};
 const steam=game.launcher==="Steam";
 const title=sandbox?(tx("ui-mako-launch-setup", "prepare_mako_in_flatpak")):steam?(tx("ui-mako-launch-setup", "apply_steam_launch_options")):(tx("ui-mako-launch-setup", "start_this_game_with_mako"));
 if(!sandbox&&!steam)return <PeliRunProfile game={game} setupOnly onClose={onClose}/>;
 return <NexusPopup className="mako-launch-setup" title={title} titleIcon={<MenuIcon name="settings"/>} onClose={()=>{if(!busy)onClose()}} footer={<>
  <button type="button" className="modern-mini-btn" disabled={busy} onClick={onClose}>{tx("ui-mako-launch-setup", "close")}</button>
  {sandbox?<button type="button" className="modern-mini-btn" disabled={busy} onClick={()=>{setBusy(true);void onCheckSandbox?.().finally(()=>setBusy(false))}}>{tx("ui-mako-launch-setup", "verify_setup")}</button>:steam?<><button type="button" className="modern-mini-btn" disabled={busy||!line} onClick={()=>void apply("confirm-manual")}>{tx("ui-mako-launch-setup", "i_added_it_verify")}</button><button type="button" className="modern-mini-btn" disabled={busy} onClick={()=>void apply("restart")}>{busy?(tx("ui-mako-launch-setup", "please_wait")):(tx("ui-mako-launch-setup", "restart_steam_and_apply"))}</button></>:null}
  <button type="button" className="modern-mini-btn" disabled={!sandbox&&!line} onClick={()=>void copy()}><MenuIcon name={copied?"check":"copy"}/>{copied?(tx("ui-mako-launch-setup", "copied")):(tx("ui-mako-launch-setup", "copy"))}</button>
 </>}>
 {sandbox?<>
  <p>{sandbox.message}</p><p><strong>{sandbox.app_id}</strong><br/>{sandbox.runtime}{sandbox.branch?` · ABI ${sandbox.branch}`:""}</p>
  {!sandbox.ready&&sandbox.branch&&<><p>{tx("ui-mako-launch-setup", "1_download_mako_renderer_v_version_flatpaks_tar_xz_from_the_release_below_matching_the_v")}</p><p>{tx("ui-mako-launch-setup", "2_close_the_launcher_and_its_games_these_commands_install_the_runtime_extension_and_gran")}</p><textarea readOnly rows={5} value={sandbox.commands.join("\n")} aria-label={tx("ui-mako-launch-setup", "flatpak_preparation_commands")}/></>}
  <button type="button" className="modern-mini-btn" onClick={()=>void openUrl(sandbox.release_url).catch(e=>setError(String(e)))}>{tx("ui-mako-launch-setup", "official_mako_packages")}</button>
  <p>{tx("ui-mako-launch-setup", "3_verify_setup_then_enable_this_game_and_save_steam_flatpak_still_needs_the_per_game_lau")}</p>
 </>:steam?<>
  <p>{enabled?(tx("ui-mako-launch-setup", "saving_a_profile_alone_does_not_activate_mako_the_next_launch_must_use_the_command_below")):(tx("ui-mako-launch-setup", "removing_mako_also_requires_updating_steam_launch_options"))}</p>
  <p>{tx("ui-mako-launch-setup", "steam_game_properties_general_launch_options_you_can_paste_the_line_while_steam_is_open_")}</p>
  <textarea readOnly spellCheck={false} rows={4} value={line} aria-label={tx("ui-mako-launch-setup", "mako_steam_launch_command")} onFocus={e=>e.currentTarget.select()}/>
  <p>{tx("ui-mako-launch-setup", "add_this_command_once_the_helper_reads_your_latest_profile_whenever_the_game_starts_with")}</p>
 </>:null}
 {error&&<p role="alert">{error}</p>}
 </NexusPopup>;
}
