import type {Catalog} from "../../i18n/engine";
import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {useGameSaveStatus,GameSaveStatusIndicator} from "../../components/GameSaveStatus";
import {DebouncedSave} from "../../services/debouncedSave";
import {openFrameGenerationPreferences} from "../../services/frameGenerationNavigation";
import {ModOptionSwitch} from "../../components/ModOptionRow";
import type {GameInfo} from "../../components/GameGrid";
import {defaultAfmf,isFrameGenerationWrapper,normalizeAdvanced,type AdvancedSettings,type AfmfSettings} from "../../services/advancedSettings";
import {selectFrameGenerator} from "../../services/frameGenerationSelection";
import {updatePeliGamesSettings} from "../../services/installedLibrary";
import {useI18n} from "../../i18n/I18nContext";
import "./frameGeneration.css";

type Runtime={installed_versions:{version:string;current:boolean;directory:string}[];current_version:string;install_version:string;available_version:string;update_available:boolean;update_compatible:boolean;install_directory:string};
type Support={available:boolean;reason:string};
type Choice<T extends string>={value:T;labelKey:keyof Catalog["ui-afmf-panel"]};

const MODES:Choice<AfmfSettings["performance_mode"]>[]=[{value:"auto", labelKey: "afmf_auto_0"},{value:"quality", labelKey: "afmf_quality_0"},{value:"performance", labelKey: "afmf_performance_0"}];
const SEARCH:Choice<AfmfSettings["search_mode"]>[]=[{value:"auto", labelKey: "afmf_auto_0"},{value:"standard", labelKey: "afmf_standard_0"},{value:"high", labelKey: "afmf_high_0"}];
const MOTION:Choice<AfmfSettings["fast_motion"]>[]=[{value:"blend", labelKey: "afmf_blend_0"},{value:"repeat", labelKey: "afmf_repeat_0"}];

/** AFMF for one game: a guided setup (install → enable → relaunch) and its options, saved automatically. */
export function AfmfPanel({game,disabled,running=false,onSaved,onPersist}:{game:GameInfo;disabled:boolean;running?:boolean;onSaved:(game:GameInfo)=>void;onPersist?:(settings:AdvancedSettings)=>Promise<void>}){
  const {t: tx} = useI18n();

 const advanced=normalizeAdvanced(game.advanced);
 const [settings,setSettings]=useState<AfmfSettings>(()=>advanced.afmf??defaultAfmf);
 const [runtime,setRuntime]=useState<Runtime>();const [support,setSupport]=useState<Support>();
 const [busy,setBusy]=useState(false);const [error,setError]=useState("");
 const [saveStatus,setSaveStatus]=useState<""|"pending"|"saving"|"saved"|"error">("");const [savePanel,setSavePanel]=useState<"afmf-prepare"|"afmf-options">("afmf-prepare");
 const headerStatus=useGameSaveStatus(saveStatus,savePanel);
 const autosave=useRef(new DebouncedSave());const lock=useRef(false);
 useEffect(()=>()=>{void autosave.current.flush();},[]);
 const key=game.external_id||game.path;
 useEffect(()=>{setSettings(normalizeAdvanced(game.advanced).afmf??defaultAfmf)},[key,game.advanced?.afmf]);
 const loadRuntime=()=>invoke<Runtime>("frame_generation_state",{gameKey:"runtime-preferences:afmf",backend:"afmf",activeProfile:null}).then(setRuntime).catch(e=>setError(String(e)));
 useEffect(()=>{void loadRuntime();const reload=()=>void loadRuntime();window.addEventListener("frameGenerationRuntimeChanged",reload);return()=>window.removeEventListener("frameGenerationRuntimeChanged",reload)},[]);
 useEffect(()=>{let live=true;setSupport(undefined);void invoke<Support>("afmf_support",{directory:game.directory||game.path,executable:game.executable||null,flatpak:Boolean(game.flatpak)}).then(s=>{if(live)setSupport(s)}).catch(e=>{if(live)setSupport({available:false,reason:String(e)})});return()=>{live=false}},[key,game.directory,game.executable,game.flatpak]);

 const installed=Boolean(runtime?.installed_versions.some(r=>r.current));
 const supported=Boolean(onPersist)||game.launcher==="PeliGames"||Boolean(game.external_id);
 const makoOn=(advanced.wrappers??[]).some(w=>isFrameGenerationWrapper(w)&&w.enabled);
 const conflict=(advanced.options.mlfg||advanced.options.optiscaler)?(tx("ui-afmf-panel", "disable_ml_frame_generation_and_optiscaler_in_the_advanced_tab_before_enabling_afmf")):"";
 const blocked=busy||disabled||running;

 const persist=async(next:AfmfSettings)=>{if(lock.current)return;lock.current=true;setBusy(true);setSaveStatus("saving");try{
  const nextAdvanced=next.enabled?selectFrameGenerator({...advanced,afmf:next},"afmf"):{...advanced,afmf:next};
  const updated=onPersist?await onPersist(nextAdvanced).then(()=>({...game,advanced:nextAdvanced})):game.external_id?await invoke<GameInfo>("configure_external_game",{settings:{external_id:game.external_id,advanced:nextAdvanced}}):await updatePeliGamesSettings({path:game.path,name:game.name,executable:game.executable||game.path,proton:game.proton||"",prefix:game.prefix,advanced:nextAdvanced});
  onSaved({...game,...updated,library_view:game.library_view});window.dispatchEvent(new Event("peligamesLibraryChanged"));setSaveStatus("saved");setError("");
 }catch(e){setSaveStatus("error");setError(String(e))}finally{lock.current=false;setBusy(false)}};
 const patch=(value:Partial<AfmfSettings>,panel:"afmf-prepare"|"afmf-options"="afmf-options")=>{const next={...settings,...value};setSettings(next);setSavePanel(panel);setSaveStatus("pending");setError("");autosave.current.schedule(()=>persist(next));};

 const segments=<T extends string>(label:string,choices:Choice<T>[],value:T,onPick:(v:T)=>void)=><div className="modern-dlss-segments fg-multipliers" role="group" aria-label={label}>{choices.map(c=><button type="button" key={c.value} aria-pressed={value===c.value} disabled={blocked} onClick={()=>onPick(c.value)}>{tx("ui-afmf-panel", c.labelKey)}</button>)}</div>;
 const canEnable=installed&&supported&&Boolean(support?.available)&&!conflict;

 return <section className="frame-generation-panel fg-game-workflow afmf-panel">
  <div className="modern-dlss-panels fg-game-grid">
   <section className="modern-dlss-card fg-game-setup" aria-label={tx("ui-afmf-panel", "prepare_afmf")}>
    <div className="modern-panel-heading"><h3>{tx("ui-afmf-panel", "prepare_for_this_game")}</h3><GameSaveStatusIndicator scope="afmf-prepare"/></div>
    <p className="modern-dlss-description">{tx("ui-afmf-panel", "afmf_generates_one_in_between_frame_per_real_frame_2_from_the_game_s_image_follow_the_st")}</p>

    <div className="afmf-steps">
     {!installed && <div className="afmf-step">
      <div className="mod-step-heading"><div><h4>{tx("ui-afmf-panel", "prepare_the_component")}</h4><p>{tx("ui-afmf-panel", "install_afmf_in_settings_add_ons_the_installation_is_shared_across_games")}</p></div></div>
      {runtime ? <div className="fg-component-row"><button type="button" className="modern-mini-btn" disabled={blocked} onClick={()=>openFrameGenerationPreferences("afmf")}>{tx("ui-afmf-panel", "install")}</button></div> : <p role="status">{tx("ui-afmf-panel", "checking_component")}</p>}
     </div>}

     <div className="afmf-step">
      <div className="mod-step-heading"><div><h4>{tx("ui-afmf-panel", "enable_for_this_game")}</h4><p>{support?.reason??(tx("ui-afmf-panel", "checking_the_game_s_compatibility"))}</p></div></div>
      <div className="fg-toolbar"><div className="fg-master-toggle"><span>{tx("ui-afmf-panel", "enable_afmf")}</span><ModOptionSwitch label={tx("ui-afmf-panel", "enable_afmf_for_this_game")} enabled={settings.enabled} disabled={blocked||(!settings.enabled&&!canEnable)} onToggle={()=>patch({enabled:!settings.enabled},"afmf-prepare")}/></div><span className="fg-generator-name">AFMF · 2×</span></div>
      {!installed&&runtime&&<p className="mod-step-note">{tx("ui-afmf-panel", "install_afmf_in_settings_add_ons_to_unlock_activation")}</p>}
      {makoOn&&<p className="mod-step-note">{tx("ui-afmf-panel", "enabling_afmf_automatically_disables_mako_for_this_game")}</p>}
      {conflict&&<p className="fg-partition-note" role="alert">{conflict}</p>}
      {!supported&&<p className="mod-step-note">{tx("ui-afmf-panel", "register_this_installation_in_the_main_library_to_configure_its_launch")}</p>}
     </div>

     <div className="afmf-step">
      <div className="mod-step-heading"><div><h4>{tx("ui-afmf-panel", "launch_or_relaunch_the_game")}</h4><p>{tx("ui-afmf-panel", "afmf_reads_its_options_when_the_game_starts_any_change_applies_on_the_next_launch")}</p></div></div>
      <p className="mod-step-note">{tx("ui-afmf-panel", "with_the_peligames_overlay_on_performance_tab_open_the_in_game_panel_with_your_configure")}</p>
      {game.steam_pending&&<p className="mod-step-note">{tx("ui-afmf-panel", "steam_has_pending_changes_apply_them_beside_the_play_button")}</p>}
      {running&&<p role="status">{tx("ui-afmf-panel", "game_running_close_it_to_change_afmf")}</p>}
     </div>
    </div>

    {!headerStatus&&<span role="status" className="fg-autosave-status">{saveStatus==="saving"?(tx("ui-afmf-panel", "saving")):saveStatus==="error"?(tx("ui-afmf-panel", "could_not_save")):saveStatus==="saved"?(tx("ui-afmf-panel", "saved_automatically")):(tx("ui-afmf-panel", "changes_save_automatically"))}</span>}
    {error&&<p className="mod-feedback-error" role="alert">{error}</p>}
   </section>

   <section className="modern-dlss-card fg-game-options" aria-label={tx("ui-afmf-panel", "afmf_options")}>
    <div className="modern-panel-heading"><h3>{tx("ui-afmf-panel", "options")}</h3><GameSaveStatusIndicator scope="afmf-options"/></div>
    <p className="modern-dlss-description">{tx("ui-afmf-panel", "defaults_work_for_most_games_adjust_if_you_notice_artifacts_or_a_performance_drop")}</p>
    <div className="fg-controls">
     <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{tx("ui-afmf-panel", "mode")}</span><small>{tx("ui-afmf-panel", "quality_reduces_artifacts_performance_costs_the_gpu_less")}</small></span>{segments(tx("ui-afmf-panel", "mode"),MODES,settings.performance_mode,v=>patch({performance_mode:v}))}</div>
     <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{tx("ui-afmf-panel", "motion_search")}</span><small>{tx("ui-afmf-panel", "high_follows_wide_motion_at_a_higher_cost")}</small></span>{segments(tx("ui-afmf-panel", "motion_search"),SEARCH,settings.search_mode,v=>patch({search_mode:v}))}</div>
     <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{tx("ui-afmf-panel", "fast_motion")}</span><small>{tx("ui-afmf-panel", "when_motion_is_too_fast_blend_smooths_repeat_avoids_warping")}</small></span>{segments(tx("ui-afmf-panel", "fast_motion"),MOTION,settings.fast_motion,v=>patch({fast_motion:v}))}</div>
     <label className="modern-dlss-setting fg-control-field fg-setting"><span className="fg-setting-copy"><span>{tx("ui-afmf-panel", "minimum_fps_to_generate")}</span><small>{tx("ui-afmf-panel", "below_this_real_fps_no_frame_is_generated_0_always_generates")}</small></span><input type="number" min={0} max={240} value={settings.min_fps} disabled={blocked} onChange={e=>{const v=Math.max(0,Math.min(240,Math.round(Number(e.target.value)||0)));patch({min_fps:v})}}/></label>
     <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{tx("ui-afmf-panel", "detect_hud")}</span><small>{tx("ui-afmf-panel", "keeps_menus_and_hud_still_without_warping")}</small></span><ModOptionSwitch label={tx("ui-afmf-panel", "detect_hud")} enabled={settings.hud_detect} disabled={blocked} onToggle={()=>patch({hud_detect:!settings.hud_detect})}/></div>
     <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{tx("ui-afmf-panel", "frame_pacing")}</span><small>{tx("ui-afmf-panel", "spaces_frames_evenly_holds_each_real_frame_slightly")}</small></span><ModOptionSwitch label={tx("ui-afmf-panel", "frame_pacing")} enabled={settings.pacing} disabled={blocked} onToggle={()=>patch({pacing:!settings.pacing})}/></div>
    </div>
    {advanced.gamescope?.enable_upscaling||advanced.gamescope?.enable_limiter||advanced.gamescope?.force_grab_cursor||advanced.gamescope?.additional_options?.trim()?<p className="fg-overlay-support-note">{tx("ui-afmf-panel", "gamescope_detected_afmf_s_compatible_mode_is_enabled_automatically")}</p>:null}
    <div className="fg-compatibility"><strong>{tx("ui-afmf-panel", "compatibility")}</strong>
     <p>{tx("ui-afmf-panel", "tested_on_amd_rx_7000_and_rx_9000_cards_with_the_radv_driver_other_cards_are_untested_re")}</p>
     <p>{tx("ui-afmf-panel", "fixed_2_multiplier_a_game_already_at_the_monitor_s_refresh_rate_with_v_sync_gets_no_gene")}</p>
     <p>{tx("ui-afmf-panel", "do_not_combine_with_mako_lsfg_vk_or_optiscaler_frame_generation_mangohud_may_count_only_")}</p>
    </div>
   </section>
  </div>
 </section>;
}
