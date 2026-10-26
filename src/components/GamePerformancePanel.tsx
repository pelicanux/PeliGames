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
      <div className="modern-panel-heading"><h3>{pt ? "Monitoramento e FPS" : "Monitoring and FPS"}</h3><GameSaveStatusIndicator/></div>
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
      <div className="modern-performance-row"><label htmlFor={`${id}-fps`}>{settings.afmf?.enabled ? (pt ? "Limitar FPS total" : "Limit total FPS") : (pt ? "Limitar FPS" : "Limit FPS")}</label><div className="modern-performance-limits">
        <div className="modern-dlss-segments" role="group" aria-label={pt ? "Limitar FPS" : "Limit FPS"}>{[0,60,120,144].map(n => <button type="button" key={n} disabled={blocked} aria-pressed={settings.mangohud_fps_limit === n} onClick={()=>limit(n)}>{n || (pt ? "Sem limite" : "Unlimited")}</button>)}</div>
        <input id={`${id}-fps`} type="number" min="0" max="1000" step="1" value={fps} placeholder={pt ? "Padrão" : "Default"} aria-label={pt ? "Limite de FPS personalizado" : "Custom FPS limit"} disabled={blocked} onChange={e=>setFps(e.target.value)} onBlur={()=>{if(fps !== String(settings.mangohud_fps_limit ?? ""))limit(fps.trim() ? Number(fps) : null);}} onKeyDown={e=>{if(e.key === "Enter")e.currentTarget.blur();}}/>
      </div></div>
      <p className="modern-performance-hint">{pt ? "O limite vale só para este jogo, mesmo com o painel oculto. Campo vazio usa o limite geral." : "The limit applies only to this game, even with the HUD hidden. Leave blank to use the global limit."}</p>
      {experimentalFrameGeneration && settings.afmf?.enabled && <p className="modern-performance-hint">{pt ? "Com AFMF, o MangoHUD mede e limita o total, incluindo os quadros gerados." : "With AFMF, MangoHUD measures and limits the total, including generated frames."}</p>}
      {experimentalFrameGeneration&&<>
      <div className="modern-performance-row"><div><label htmlFor={`${id}-background`}>{pt?"Limitar FPS sem foco":"Limit FPS when unfocused"}</label><p>{pt?"Usa o MangoHUD com o overlay carregado, mesmo com os painéis ocultos.":"Uses MangoHUD while the overlay is loaded, even with both panels hidden."}</p></div><button id={`${id}-background`} type="button" role="switch" aria-checked={settings.background_fps!=null} className={`neural-startup-switch mod-option-switch${settings.background_fps!=null?" enabled":""}`} disabled={blocked||!overlayEnabled} onClick={()=>void commit({...settings,overlay_enabled:true,background_fps:settings.background_fps==null?0:null})}><span className="neural-switch-track"><span/></span></button></div>
      {settings.background_fps!=null&&<div className="modern-performance-row"><label htmlFor={`${id}-background-fps`}>{pt?"FPS sem foco":"Background FPS"}</label><div className="modern-performance-limits"><div className="modern-dlss-segments">{[0,30,60].map(n=><button type="button" key={n} disabled={blocked} aria-pressed={settings.background_fps===n} onClick={()=>void commit({...normalizeAdvanced(latest.current),background_fps:n})}>{n||(pt?"Automático":"Automatic")}</button>)}</div><input id={`${id}-background-fps`} type="number" min="1" max="1000" step="1" value={backgroundFps==="0"?"":backgroundFps} placeholder={pt?"FPS personalizado":"Custom FPS"} disabled={blocked} aria-label={pt?"FPS sem foco personalizado":"Custom background FPS"} onChange={e=>setBackgroundFps(e.target.value)} onBlur={()=>{if(backgroundFps!==String(settings.background_fps)&&backgroundFps.trim())void commit({...normalizeAdvanced(latest.current),background_fps:Number(backgroundFps)});}} onKeyDown={e=>{if(e.key==="Enter")e.currentTarget.blur();}}/></div></div>}
      <p className="modern-performance-hint">{pt?"Automático usa o menor limite positivo do MangoHUD (ou 30 FPS se não houver). Ao voltar à janela, restaura o limite configurado. Cada processo tem seu próprio perfil temporário. Experimental: exige uma rota de foco compatível e Vulkan. Salva automaticamente; reinicie o jogo para aplicar.":"Automatic uses MangoHUD’s lowest positive limit (or 30 FPS if none). Restores the configured limit when returning. Each process has its own temporary profile. Experimental: requires compatible focus detection and Vulkan. Saves automatically; relaunch the game to apply."}</p>
      </>}
      <div className="modern-performance-settings-link"><button type="button" className="modern-mini-btn" onClick={openMonitoringPreferences}>{pt?"Configurar MangoHUD":"Configure MangoHUD"}</button></div>
      {busy && !headerStatus && <p role="status">{pt ? "Salvando…" : "Saving…"}</p>}
      {(error || profileError) && <p role="alert">{error || profileError}</p>}
    </section>

    {(openBenchmarks || experimentalFrameGeneration) && <section className="modern-performance-card modern-game-benchmark-card" aria-label={pt?"Benchmarks e ferramentas":"Benchmarks and tools"}>
      {openBenchmarks && <>
      <div className="modern-game-benchmark-copy"><h3>{pt?"Benchmarks deste jogo":"Game benchmarks"}</h3><p>{pt?"Compare duas gravações do MangoHUD para avaliar o FPS e a fluidez deste jogo. Veja o resultado de mudanças no Proton, nos gráficos ou nos ajustes de desempenho.":"Compare two MangoHUD recordings to evaluate this game's FPS and smoothness. Check the effect of changes to Proton, graphics or performance settings."}</p><small>{pt?"Grave o mesmo trecho nas duas condições para uma comparação consistente.":"Record the same scene under both conditions for a consistent comparison."}</small></div>
      <button type="button" className="modern-mini-btn modern-benchmark-action" data-game-benchmark-open onClick={openBenchmarks}>{pt?"Comparar testes deste jogo":"Compare this game’s tests"}</button>
      </>}
      {experimentalFrameGeneration && <div className="modern-performance-tools"><h3>{pt?"Ferramentas durante o jogo":"In-game tools"}</h3>
      <div className="modern-performance-row"><div><label htmlFor={`${id}-overlay`}>Overlay PeliGames</label><p>{pt?"MAKO e FPS sem foco no jogo. Inicia oculto; abra pelo atalho abaixo.":"In-game MAKO and unfocused FPS controls. Starts hidden; open with the shortcut below."}</p></div><button id={`${id}-overlay`} type="button" role="switch" aria-checked={overlayEnabled} className={`neural-startup-switch mod-option-switch${overlayEnabled?" enabled":""}`} disabled={blocked||(!overlayEnabled&&!overlaySupport?.available)} onClick={()=>void commit({...settings,overlay_enabled:!overlayEnabled,background_fps:overlayEnabled?null:settings.background_fps})}><span className="neural-switch-track"><span/></span></button></div>
      {overlaySupport?.reason && <p className="modern-performance-hint">{overlaySupport.available ? (pt?"Experimental: exige Vulkan de 64 bits. No Linux nativo, a interação requer Wayland. Windows de 32 bits pode exigir WoW64 em Avançado; se falhar, desative. Linux de 32 bits não é suportado.":"Experimental: requires 64-bit Vulkan. Native Linux input requires Wayland. 32-bit Windows games may need WoW64 in Advanced; disable it if it fails. 32-bit Linux is unsupported.") : overlaySupport.reason}{overlaySupport.available && game?.flatpak && (pt?" Na Steam Flatpak, o overlay é preparado dentro do sandbox.":" On Flatpak Steam, the overlay is prepared inside the sandbox.")}</p>}
      {overlayEnabled&&overlaySession&&<div className="modern-overlay-session" data-level={overlaySession.level} role="status"><strong>{pt?"Última execução":"Last launch"} · {new Date(overlaySession.started*1000).toLocaleString(pt?"pt-BR":undefined)}</strong><div>{overlaySession.summary}</div>{overlaySession.lines.length>0&&<details><summary>{pt?"Etapas registradas":"Recorded steps"}</summary><pre>{overlaySession.lines.join("\n")}</pre></details>}</div>}
      <FrameOverlaySettings compact disabled={blocked}/>
      </div>}
    </section>}

  </div>;
}
