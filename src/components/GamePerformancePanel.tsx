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
import { Hud } from "./preferences/ModernMangoHudEditor";
import { useI18n } from "../i18n/I18nContext";
import { usePerformanceMode } from "./EffectsContext";

export function GamePerformancePanel({value, disabled, onChange, game}: {
  game?: {path:string;directory?:string;executable?:string;flatpak?:boolean};
  value?: AdvancedSettings; disabled?: boolean; onChange: (settings: AdvancedSettings) => Promise<boolean>;
}) {
  const openBenchmarks = useContext(GameBenchmarkNavigationContext);
  const {language} = useI18n(), pt = language === "pt", id = useId();
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
  useEffect(()=>{let current=true;setOverlaySupport(undefined);if(game)void invoke<{available:boolean;reason:string;legacy_enabled?:boolean}>("frame_overlay_support",{directory:game.directory||game.path,executable:game.executable||null,flatpak:Boolean(game.flatpak),advanced:value}).then(s=>{if(current)setOverlaySupport(s)}).catch(e=>{if(current)setOverlaySupport({available:false,reason:String(e)})});return()=>{current=false}},[game?.directory,game?.path,game?.executable,game?.flatpak,value?.wrappers]);
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
      if(next.overlay_enabled && !overlaySupport?.available)throw new Error(overlaySupport?.reason||(pt?"Aguarde a verificação do overlay.":"Wait for the overlay compatibility check."));
      if (next.background_fps!=null || next.mangohud_enabled || (next.mangohud_fps_limit ?? 0) > 0) {
        if (!(await getMangoHudStatus()).installed) {if(alive.current)notifyMangoHudMissing();return;}
      }
      if (!alive.current || latest.current !== initial) return;
      const success = await onChange(next);
      if (alive.current) {setSaved(success);if(!success)setError(pt ? "Não foi possível salvar os ajustes deste jogo." : "Could not save this game's settings.");}
    } catch(e) {if(alive.current)setError(String(e));}
    finally {lock.current = false;if(alive.current)setBusy(false);}
  };
  const limit = (n: number | null) => {setFps(n === null ? "" : String(n));void commit({...normalizeAdvanced(latest.current), mangohud_fps_limit:n});};
  const blocked = disabled || busy || !loaded;
  return <div className="modern-performance-panels">
    <section className="modern-performance-card">
      <div className="modern-panel-heading"><h3>{pt ? "Neste jogo" : "For this game"}</h3><GameSaveStatusIndicator/></div>
      <div className="modern-performance-row"><div><label htmlFor={`${id}-hud`}>{pt ? "Mostrar monitoramento (MangoHUD)" : "Show monitoring (MangoHUD)"}</label><p>{pt ? "FPS, temperatura e uso de hardware" : "FPS, temperature and hardware usage"}</p></div>
        <button id={`${id}-hud`} type="button" role="switch" aria-checked={enabled} aria-label={pt ? "Mostrar monitoramento (MangoHUD)" : "Show monitoring (MangoHUD)"} className={`neural-startup-switch mod-option-switch${enabled ? " enabled" : ""}`} disabled={blocked} onClick={() => void commit({...settings, mangohud_enabled:!enabled, options:{...settings.options,mangohud:!enabled,mangohud_env:false}})}><span className="neural-switch-track"><span/></span></button>
      </div>
      <div className="modern-performance-row"><div><label>{pt ? "Preset do MangoHUD" : "MangoHUD preset"}</label><p>{pt ? "Aparência exclusiva deste jogo. Não altera o perfil global nem os limites de FPS." : "Appearance for this game only. Does not change the global profile or FPS limits."}</p></div>
        <SelectionMenu label={pt ? "Preset do MangoHUD" : "MangoHUD preset"} className="modern-game-preset-select" minMenuWidth={240} value={settings.mangohud_preset ? "selected" : "global"} disabled={blocked} onChange={choice=>{
          if(choice==="global"){void commit({...settings,mangohud_preset:null});return;}
          const builtin=choice.startsWith("builtin:")?PRESETS.find(p=>p.id===choice.slice(8)):undefined;
          const preset=builtin?{name:builtin.name,settings:applyPreset(builtin.id,profile)}:presets.find(p=>`custom:${p.name}`===choice);
          if(preset)void commit({...settings,mangohud_preset:{name:preset.name,settings:{...preset.settings}}});
        }} options={[
          {value:"global",label:pt ? "Usar perfil global" : "Use global profile"},
          ...(settings.mangohud_preset?[{value:"selected",label:settings.mangohud_preset.name}]:[]),
          ...PRESETS.map(p=>({value:`builtin:${p.id}`,label:p.name})),
          ...presets.map(p=>({value:`custom:${p.name}`,label:p.name})),
        ]}/>

      </div>
      {presetError&&<p role="alert">{presetError}</p>}
      <div className="modern-performance-row"><label htmlFor={`${id}-fps`}>{pt ? "Limitar FPS" : "Limit FPS"}</label><div className="modern-performance-limits">
        <div className="modern-dlss-segments" role="group" aria-label={pt ? "Limitar FPS" : "Limit FPS"}>{[0,60,120,144].map(n => <button type="button" key={n} disabled={blocked} aria-pressed={settings.mangohud_fps_limit === n} onClick={()=>limit(n)}>{n || (pt ? "Sem limite" : "Unlimited")}</button>)}</div>
        <input id={`${id}-fps`} type="number" min="0" max="1000" step="1" value={fps} placeholder={pt ? "Padrão" : "Default"} aria-label={pt ? "Limite de FPS personalizado" : "Custom FPS limit"} disabled={blocked} onChange={e=>setFps(e.target.value)} onBlur={()=>{if(fps !== String(settings.mangohud_fps_limit ?? ""))limit(fps.trim() ? Number(fps) : null);}} onKeyDown={e=>{if(e.key === "Enter")e.currentTarget.blur();}}/>
      </div></div>
      <p className="modern-performance-hint">{pt ? "O limite vale só para este jogo, mesmo com o painel oculto. Campo vazio usa o limite geral." : "The limit applies only to this game, even with the HUD hidden. Leave blank to use the global limit."}</p>
      {experimentalFrameGeneration&&<>
      <div className="modern-performance-row"><div><label htmlFor={`${id}-overlay`}>Overlay PeliGames</label><p>{pt?"Uma única camada para MAKO, FPS sem foco e outras ferramentas. Detecta as funções ativas e inicia oculta.":"One overlay for MAKO, unfocused FPS and other tools. Detects enabled features and starts hidden."}</p></div><button id={`${id}-overlay`} type="button" role="switch" aria-checked={overlayEnabled} className={`neural-startup-switch mod-option-switch${overlayEnabled?" enabled":""}`} disabled={blocked||(!overlayEnabled&&!overlaySupport?.available)} onClick={()=>void commit({...settings,overlay_enabled:!overlayEnabled,background_fps:overlayEnabled?null:settings.background_fps})}><span className="neural-switch-track"><span/></span></button></div>
      <p className="modern-performance-hint">{overlaySupport?.reason}</p>
      <div className="modern-performance-row"><div><label htmlFor={`${id}-background`}>{pt?"Limitar FPS sem foco":"Limit FPS when unfocused"}</label><p>{pt?"Usa o MangoHUD com o overlay carregado, mesmo com os painéis ocultos.":"Uses MangoHUD while the overlay is loaded, even with both panels hidden."}</p></div><button id={`${id}-background`} type="button" role="switch" aria-checked={settings.background_fps!=null} className={`neural-startup-switch mod-option-switch${settings.background_fps!=null?" enabled":""}`} disabled={blocked||!overlayEnabled} onClick={()=>void commit({...settings,overlay_enabled:true,background_fps:settings.background_fps==null?0:null})}><span className="neural-switch-track"><span/></span></button></div>
      {settings.background_fps!=null&&<div className="modern-performance-row"><label htmlFor={`${id}-background-fps`}>{pt?"FPS sem foco":"Background FPS"}</label><div className="modern-performance-limits"><div className="modern-dlss-segments">{[0,30,60].map(n=><button type="button" key={n} disabled={blocked} aria-pressed={settings.background_fps===n} onClick={()=>void commit({...normalizeAdvanced(latest.current),background_fps:n})}>{n||(pt?"Automático":"Automatic")}</button>)}</div><input id={`${id}-background-fps`} type="number" min="1" max="1000" step="1" value={backgroundFps==="0"?"":backgroundFps} placeholder={pt?"FPS personalizado":"Custom FPS"} disabled={blocked} aria-label={pt?"FPS sem foco personalizado":"Custom background FPS"} onChange={e=>setBackgroundFps(e.target.value)} onBlur={()=>{if(backgroundFps!==String(settings.background_fps)&&backgroundFps.trim())void commit({...normalizeAdvanced(latest.current),background_fps:Number(backgroundFps)});}} onKeyDown={e=>{if(e.key==="Enter")e.currentTarget.blur();}}/></div></div>}
      <p className="modern-performance-hint">{pt?"Automático usa o menor limite positivo do MangoHUD (ou 30 FPS se não houver). Ao voltar à janela, restaura o limite configurado. Cada processo tem seu próprio perfil temporário. Experimental: exige uma rota de foco compatível e Vulkan. Salva automaticamente; reinicie o jogo para aplicar.":"Automatic uses MangoHUD’s lowest positive limit (or 30 FPS if none). Restores the configured limit when returning. Each process has its own temporary profile. Experimental: requires compatible focus detection and Vulkan. Saves automatically; relaunch the game to apply."}</p>
      <FrameOverlaySettings compact disabled={blocked}/>
      </>}
      <div className="modern-performance-visual"><p>{pt ? "O visual do painel (cores, posição e o que mostrar) é o mesmo para todos os jogos." : "The panel appearance (colors, position and displayed metrics) is shared by all games."}</p><div className="modern-performance-actions"><button type="button" className="modern-mini-btn" onClick={openMonitoringPreferences}>{pt ? "Configurar visual" : "Configure appearance"}</button>{openBenchmarks && <button type="button" className="modern-mini-btn" data-game-benchmark-open onClick={openBenchmarks}>{pt ? "Comparar testes deste jogo" : "Compare this game’s tests"}</button>}</div></div>
      {busy && !headerStatus && <p role="status">{pt ? "Salvando…" : "Saving…"}</p>}
      {(error || profileError) && <p role="alert">{error || profileError}</p>}
    </section>
    <section className="modern-performance-card mango-modern modern-performance-preview"><h3>{pt ? "Aparência do monitoramento" : "Monitoring appearance"}</h3>{loaded ? <MonitoringPreview settings={settings.mangohud_preset ? {...mangoHudDefaults(),...settings.mangohud_preset.settings} : profile}/> : <p role="status">{pt ? "Carregando prévia…" : "Loading preview…"}</p>}</section>
  </div>;
}
function MonitoringPreview({settings}:{settings:MangoHudSettings}) {
  const screen = useRef<HTMLDivElement>(null), reduced = usePerformanceMode();
  const [width,setWidth] = useState(480), [tick,setTick] = useState(0);
  const [graph,setGraph] = useState(()=>Array.from({length:48},()=>16));
  useEffect(()=>{if(!screen.current)return;const observer=new ResizeObserver(([e])=>setWidth(e.contentRect.width));observer.observe(screen.current);return()=>observer.disconnect();},[]);
  useEffect(()=>{if(reduced || matchMedia("(prefers-reduced-motion: reduce)").matches)return;const timer=setInterval(()=>{setTick(t=>t+1);setGraph(g=>[...g.slice(1),16+Math.random()*3]);},450);return()=>clearInterval(timer);},[reduced]);
  return <div ref={screen} className="mm-screen" aria-label="MangoHUD preview"><Hud settings={settings} tick={tick} size={Number(settings.font_size)*(width/2560)*2.6} graph={graph}/></div>;
}
