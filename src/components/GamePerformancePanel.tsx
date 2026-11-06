import {SelectionMenu} from "./SelectionMenu";
import {PRESETS, applyPreset} from "./preferences/mangoHudPresets";
import {listMangoHudPresets, type MangoHudPreset} from "../services/mangohud";
import {useGameSaveStatus, GameSaveStatusIndicator} from "./GameSaveStatus";
import {invoke} from "@tauri-apps/api/core";
import {FrameOverlaySettings} from "../modules/frameGeneration/FrameOverlaySettings";
import {experimentalFrameGeneration} from "../services/releaseFeatures";
import { GameBenchmarkNavigationContext } from "./preferences/GameBenchmarkNavigationContext";
import { useContext, useEffect, useId, useRef, useState } from "react";
import { advancedError, normalizeAdvanced, type AdvancedSettings } from "../services/advancedSettings";
import { notifyMangoHudMissing, getMangoHudStatus, loadMangoHudProfile, openMonitoringPreferences, type MangoHudSettings } from "../services/mangohud";
import { mangoHudDefaults } from "./preferences/mangoHudDefaults";
import { useI18n } from "../i18n/I18nContext";

export function GamePerformancePanel({value, disabled, onChange, game}: {
  game?: {path:string;directory?:string;executable?:string;flatpak?:boolean;external_id?:string};
  value?: AdvancedSettings; disabled?: boolean; onChange: (settings: AdvancedSettings) => Promise<boolean>;
}) {

  const openBenchmarks = useContext(GameBenchmarkNavigationContext);
  const {t: tx, language} = useI18n(), id = useId();
  const [presets, setPresets] = useState<MangoHudPreset[]>([]);
  const [presetError, setPresetError] = useState("");
  useEffect(()=>{let active=true;void listMangoHudPresets().then(items=>{if(active)setPresets(items);}).catch(e=>{if(active)setPresetError(String(e));});return()=>{active=false};},[]);
  const [profile, setProfile] = useState<MangoHudSettings>(mangoHudDefaults);
  const [loaded, setLoaded] = useState(false), [profileError, setProfileError] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  const headerStatus = useGameSaveStatus(busy ? "saving" : error ? "error" : saved ? "saved" : "");
  const [backgroundFps,setBackgroundFps]=useState(String(value?.background_fps??30));
  const [overlaySupport,setOverlaySupport]=useState<{available:boolean;reason:string;legacy_enabled?:boolean}>();
  useEffect(()=>{let current=true;setOverlaySupport(undefined);if(game)void invoke<{available:boolean;reason:string;legacy_enabled?:boolean}>("frame_overlay_support",{directory:game.directory||game.path,executable:game.executable||null,flatpak:Boolean(game.flatpak),advanced:value,externalId:game.external_id??null}).then(s=>{if(current)setOverlaySupport(s)}).catch(e=>{if(current)setOverlaySupport({available:false,reason:String(e)})});return()=>{current=false}},[game?.directory,game?.path,game?.executable,game?.flatpak,game?.external_id,value?.wrappers]);
  const [overlaySession,setOverlaySession]=useState<{started:number;level:"ok"|"warning"|"error";summary:string;lines:string[]}|null>(null);
  useEffect(()=>{let current=true;setOverlaySession(null);const directory=game?.directory||game?.path;if(directory)void invoke<{started:number;level:"ok"|"warning"|"error";summary:string;lines:string[]}|null>("frame_overlay_last_session",{directory}).then(s=>{if(current)setOverlaySession(s)}).catch(()=>{});return()=>{current=false}},[game?.directory,game?.path]);
  useEffect(()=>setBackgroundFps(String(value?.background_fps??30)),[value?.background_fps]);
  const [fps, setFps] = useState(String(value?.mangohud_fps_limit ?? ""));
  const alive = useRef(true), lock = useRef(false), latest = useRef(value); latest.current = value;
  useEffect(() => { alive.current = true; let active = true;
    void loadMangoHudProfile().then(result => {if(active) {setProfile({...mangoHudDefaults(), ...result.settings});setLoaded(true);}}).catch(e => {if(active)setProfileError(String(e));});
    return () => {active = false; alive.current = false;};
  }, []);
  useEffect(() => {
    const updated = (event: Event) => {setProfile({...mangoHudDefaults(), ...(event as CustomEvent<MangoHudSettings>).detail});setLoaded(true);setProfileError("");};
    window.addEventListener("mangohudProfileChanged", updated);
    return () => window.removeEventListener("mangohudProfileChanged", updated);
  }, []);
  useEffect(() => setFps(String(value?.mangohud_fps_limit ?? "")), [value?.mangohud_fps_limit]);
  const settings = normalizeAdvanced(value);
  const overlayEnabled=settings.overlay_enabled??Boolean(overlaySupport?.legacy_enabled);
  const enabled = settings.mangohud_enabled ?? Boolean(settings.options.mangohud || settings.options.mangohud_env || profile.launcher_enabled);
  const commit = async (next: AdvancedSettings) => {
    if (disabled || lock.current) return;
    const issue = advancedError(next); if(issue) {setError(issue);return;}
    lock.current = true; setBusy(true);setError("");setSaved(false);
    const initial = latest.current;
    try {
      if(next.overlay_enabled && !overlaySupport?.available)throw new Error(overlaySupport?.reason||(tx("ui-game-performance-panel", "wait_for_the_overlay_compatibility_check")));
      if (next.background_fps!=null || next.mangohud_enabled || (next.mangohud_fps_limit ?? 0) > 0) {
        if (!(await getMangoHudStatus()).installed) {if(alive.current)notifyMangoHudMissing();return;}
      }
      if (!alive.current || latest.current !== initial) return;
      const success = await onChange(next);
      if (alive.current) {setSaved(success);if(!success)setError(tx("ui-game-performance-panel", "could_not_save_this_game_s_settings"));}
    } catch(e) {if(alive.current)setError(String(e));}
    finally {lock.current = false;if(alive.current)setBusy(false);}
  };
  const limit = (n: number | null) => {setFps(n === null ? "" : String(n));void commit({...normalizeAdvanced(latest.current), mangohud_fps_limit:n});};
  const blocked = disabled || busy || !loaded;
  return <div className="modern-performance-panels">

    <section className="modern-performance-card">
      <div className="modern-panel-heading"><h3>{tx("ui-game-performance-panel", "monitoring_and_fps")}</h3><GameSaveStatusIndicator/></div>
      <div className="modern-performance-row"><div><label htmlFor={`${id}-hud`}>{tx("ui-game-performance-panel", "show_monitoring_mangohud")}</label><p>{tx("ui-game-performance-panel", "fps_temperature_and_hardware_usage")}</p></div>
        <button id={`${id}-hud`} type="button" role="switch" aria-checked={enabled} aria-label={tx("ui-game-performance-panel", "show_monitoring_mangohud")} className={`neural-startup-switch mod-option-switch${enabled ? " enabled" : ""}`} disabled={blocked} onClick={() => void commit({...settings, mangohud_enabled:!enabled, options:{...settings.options,mangohud:!enabled,mangohud_env:false}})}><span className="neural-switch-track"><span/></span></button>
      </div>
      <div className="modern-performance-row"><div><label>{tx("ui-game-performance-panel", "mangohud_preset")}</label><p>{tx("ui-game-performance-panel", "appearance_for_this_game_only_does_not_change_the_global_profile_or_fps_limits")}</p></div>
        <SelectionMenu label={tx("ui-game-performance-panel", "mangohud_preset")} className="modern-game-preset-select" minMenuWidth={240} value={settings.mangohud_preset ? "selected" : "global"} disabled={blocked} onChange={choice=>{
          if(choice==="global"){void commit({...settings,mangohud_preset:null});return;}
          const builtin=choice.startsWith("builtin:")?PRESETS.find(p=>p.id===choice.slice(8)):undefined;
          const preset=builtin?{name:builtin.name,settings:applyPreset(builtin.id,profile)}:presets.find(p=>`custom:${p.name}`===choice);
          if(preset)void commit({...settings,mangohud_preset:{name:preset.name,settings:{...preset.settings}}});
        }} options={[
          {value:"global",label:tx("ui-game-performance-panel", "use_global_profile")},
          ...(settings.mangohud_preset?[{value:"selected",label:settings.mangohud_preset.name}]:[]),
          ...PRESETS.map(p=>({value:`builtin:${p.id}`,label:p.name})),
          ...presets.map(p=>({value:`custom:${p.name}`,label:p.name})),
        ]}/>

      </div>
      {presetError&&<p role="alert">{presetError}</p>}
      <div className="modern-performance-row"><label htmlFor={`${id}-fps`}>{settings.afmf?.enabled ? (tx("ui-game-performance-panel", "limit_total_fps")) : (tx("ui-game-performance-panel", "limit_fps"))}</label><div className="modern-performance-limits">
        <div className="modern-dlss-segments" role="group" aria-label={tx("ui-game-performance-panel", "limit_fps")}>{[0,60,120,144].map(n => <button type="button" key={n} disabled={blocked} aria-pressed={settings.mangohud_fps_limit === n} onClick={()=>limit(n)}>{n || (tx("ui-game-performance-panel", "unlimited"))}</button>)}</div>
        <input id={`${id}-fps`} type="number" min="0" max="1000" step="1" value={fps} placeholder={tx("ui-game-performance-panel", "default")} aria-label={tx("ui-game-performance-panel", "custom_fps_limit")} disabled={blocked} onChange={e=>setFps(e.target.value)} onBlur={()=>{if(fps !== String(settings.mangohud_fps_limit ?? ""))limit(fps.trim() ? Number(fps) : null);}} onKeyDown={e=>{if(e.key === "Enter")e.currentTarget.blur();}}/>
      </div></div>
      <p className="modern-performance-hint">{tx("ui-game-performance-panel", "the_limit_applies_only_to_this_game_even_with_the_hud_hidden_leave_blank_to_use_the_glob")}</p>
      {experimentalFrameGeneration && settings.afmf?.enabled && <p className="modern-performance-hint">{tx("ui-game-performance-panel", "with_afmf_mangohud_measures_and_limits_the_total_including_generated_frames")}</p>}
      {experimentalFrameGeneration&&<>
      <div className="modern-performance-row"><div><label htmlFor={`${id}-background`}>{tx("ui-game-performance-panel", "limit_fps_when_unfocused")}</label><p>{tx("ui-game-performance-panel", "uses_mangohud_while_the_overlay_is_loaded_even_with_both_panels_hidden")}</p></div><button id={`${id}-background`} type="button" role="switch" aria-checked={settings.background_fps!=null} className={`neural-startup-switch mod-option-switch${settings.background_fps!=null?" enabled":""}`} disabled={blocked||!overlayEnabled} onClick={()=>void commit({...settings,overlay_enabled:true,background_fps:settings.background_fps==null?0:null})}><span className="neural-switch-track"><span/></span></button></div>
      {settings.background_fps!=null&&<div className="modern-performance-row"><label htmlFor={`${id}-background-fps`}>{tx("ui-game-performance-panel", "background_fps")}</label><div className="modern-performance-limits"><div className="modern-dlss-segments">{[0,30,60].map(n=><button type="button" key={n} disabled={blocked} aria-pressed={settings.background_fps===n} onClick={()=>void commit({...normalizeAdvanced(latest.current),background_fps:n})}>{n||(tx("ui-game-performance-panel", "automatic"))}</button>)}</div><input id={`${id}-background-fps`} type="number" min="1" max="1000" step="1" value={backgroundFps==="0"?"":backgroundFps} placeholder={tx("ui-game-performance-panel", "custom_fps")} disabled={blocked} aria-label={tx("ui-game-performance-panel", "custom_background_fps")} onChange={e=>setBackgroundFps(e.target.value)} onBlur={()=>{if(backgroundFps!==String(settings.background_fps)&&backgroundFps.trim())void commit({...normalizeAdvanced(latest.current),background_fps:Number(backgroundFps)});}} onKeyDown={e=>{if(e.key==="Enter")e.currentTarget.blur();}}/></div></div>}
      <p className="modern-performance-hint">{tx("ui-game-performance-panel", "automatic_uses_mangohud_s_lowest_positive_limit_or_30_fps_if_none_restores_the_configure")}</p>
      </>}
      <div className="modern-performance-settings-link"><button type="button" className="modern-mini-btn" onClick={openMonitoringPreferences}>{tx("ui-game-performance-panel", "configure_mangohud")}</button></div>
      {busy && !headerStatus && <p role="status">{tx("ui-game-performance-panel", "saving")}</p>}
      {(error || profileError) && <p role="alert">{error || profileError}</p>}
    </section>

    {(openBenchmarks || experimentalFrameGeneration) && <section className="modern-performance-card modern-game-benchmark-card" aria-label={tx("ui-game-performance-panel", "benchmarks_and_tools")}>
      {openBenchmarks && <>
      <div className="modern-game-benchmark-copy"><h3>{tx("ui-game-performance-panel", "game_benchmarks")}</h3><p>{tx("ui-game-performance-panel", "compare_two_mangohud_recordings_to_evaluate_this_game_s_fps_and_smoothness_check_the_eff")}</p><small>{tx("ui-game-performance-panel", "record_the_same_scene_under_both_conditions_for_a_consistent_comparison")}</small></div>
      <button type="button" className="modern-mini-btn modern-benchmark-action" data-game-benchmark-open onClick={openBenchmarks}>{tx("ui-game-performance-panel", "compare_this_game_s_tests")}</button>
      </>}
      {experimentalFrameGeneration && <div className="modern-performance-tools"><h3>{tx("ui-game-performance-panel", "in_game_tools")}</h3>
      <div className="modern-performance-row"><div><label htmlFor={`${id}-overlay`}>Overlay PeliGames</label><p>{tx("ui-game-performance-panel", "in_game_mako_and_unfocused_fps_controls_starts_hidden_open_with_the_shortcut_below")}</p></div><button id={`${id}-overlay`} type="button" role="switch" aria-checked={overlayEnabled} className={`neural-startup-switch mod-option-switch${overlayEnabled?" enabled":""}`} disabled={blocked||(!overlayEnabled&&!overlaySupport?.available)} onClick={()=>void commit({...settings,overlay_enabled:!overlayEnabled,background_fps:overlayEnabled?null:settings.background_fps})}><span className="neural-switch-track"><span/></span></button></div>
      {overlaySupport?.reason && <p className="modern-performance-hint">{overlaySupport.available ? (tx("ui-game-performance-panel", "experimental_requires_64_bit_vulkan_native_linux_input_requires_wayland_32_bit_windows_g")) : overlaySupport.reason}{overlaySupport.available && game?.flatpak && (tx("ui-game-performance-panel", "on_flatpak_steam_the_overlay_is_prepared_inside_the_sandbox"))}</p>}
      {overlayEnabled&&overlaySession&&<div className="modern-overlay-session" data-level={overlaySession.level} role="status"><strong>{tx("ui-game-performance-panel", "last_launch")} · {new Date(overlaySession.started*1000).toLocaleString(language)}</strong><div>{overlaySession.summary}</div>{overlaySession.lines.length>0&&<details><summary>{tx("ui-game-performance-panel", "recorded_steps")}</summary><pre>{overlaySession.lines.join("\n")}</pre></details>}</div>}
      <FrameOverlaySettings compact disabled={blocked}/>
      </div>}
    </section>}

  </div>;
}
