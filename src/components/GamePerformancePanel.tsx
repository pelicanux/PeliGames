import { GameBenchmarkNavigationContext } from "./preferences/GameBenchmarkNavigationContext";
import { useContext, useEffect, useId, useRef, useState } from "react";
import { advancedError, normalizeAdvanced, type AdvancedSettings } from "../services/advancedSettings";
import { notifyMangoHudMissing, getMangoHudStatus, loadMangoHudProfile, openMonitoringPreferences, type MangoHudSettings } from "../services/mangohud";
import { mangoHudDefaults } from "./preferences/mangoHudDefaults";
import { Hud } from "./preferences/ModernMangoHudEditor";
import { useI18n } from "../i18n/I18nContext";
import { usePerformanceMode } from "./EffectsContext";

export function GamePerformancePanel({value, disabled, onChange}: {
  value?: AdvancedSettings; disabled?: boolean; onChange: (settings: AdvancedSettings) => Promise<boolean>;
}) {
  const openBenchmarks = useContext(GameBenchmarkNavigationContext);
  const {language} = useI18n(), pt = language === "pt", id = useId();
  const [profile, setProfile] = useState<MangoHudSettings>(mangoHudDefaults);
  const [loaded, setLoaded] = useState(false), [profileError, setProfileError] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
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
  const enabled = settings.mangohud_enabled ?? Boolean(settings.options.mangohud || settings.options.mangohud_env || profile.launcher_enabled);
  const commit = async (next: AdvancedSettings) => {
    if (disabled || lock.current) return;
    const issue = advancedError(next); if(issue) {setError(issue);return;}
    lock.current = true; setBusy(true);setError("");
    const initial = latest.current;
    try {
      if (next.mangohud_enabled || (next.mangohud_fps_limit ?? 0) > 0) {
        if (!(await getMangoHudStatus()).installed) {if(alive.current)notifyMangoHudMissing();return;}
      }
      if (!alive.current || latest.current !== initial) return;
      if (!await onChange(next) && alive.current) setError(pt ? "Não foi possível salvar os ajustes deste jogo." : "Could not save this game's settings.");
    } catch(e) {if(alive.current)setError(String(e));}
    finally {lock.current = false;if(alive.current)setBusy(false);}
  };
  const limit = (n: number | null) => {setFps(n === null ? "" : String(n));void commit({...normalizeAdvanced(latest.current), mangohud_fps_limit:n});};
  const blocked = disabled || busy || !loaded;
  return <div className="modern-performance-panels">
    <section className="modern-performance-card">
      <h3>{pt ? "Neste jogo" : "For this game"}</h3>
      <div className="modern-performance-row"><div><label htmlFor={`${id}-hud`}>{pt ? "Mostrar monitoramento (MangoHUD)" : "Show monitoring (MangoHUD)"}</label><p>{pt ? "FPS, temperatura e uso de hardware" : "FPS, temperature and hardware usage"}</p></div>
        <button id={`${id}-hud`} type="button" role="switch" aria-checked={enabled} aria-label={pt ? "Mostrar monitoramento (MangoHUD)" : "Show monitoring (MangoHUD)"} className={`neural-startup-switch mod-option-switch${enabled ? " enabled" : ""}`} disabled={blocked} onClick={() => void commit({...settings, mangohud_enabled:!enabled, options:{...settings.options,mangohud:!enabled,mangohud_env:false}})}><span className="neural-switch-track"><span/></span></button>
      </div>
      <div className="modern-performance-row"><label htmlFor={`${id}-fps`}>{pt ? "Limitar FPS" : "Limit FPS"}</label><div className="modern-performance-limits">
        <div className="modern-dlss-segments" role="group" aria-label={pt ? "Limitar FPS" : "Limit FPS"}>{[0,60,120,144].map(n => <button type="button" key={n} disabled={blocked} aria-pressed={settings.mangohud_fps_limit === n} onClick={()=>limit(n)}>{n || (pt ? "Sem limite" : "Unlimited")}</button>)}</div>
        <input id={`${id}-fps`} type="number" min="0" max="1000" step="1" value={fps} placeholder={pt ? "Padrão" : "Default"} aria-label={pt ? "Limite de FPS personalizado" : "Custom FPS limit"} disabled={blocked} onChange={e=>setFps(e.target.value)} onBlur={()=>{if(fps !== String(settings.mangohud_fps_limit ?? ""))limit(fps.trim() ? Number(fps) : null);}} onKeyDown={e=>{if(e.key === "Enter")e.currentTarget.blur();}}/>
      </div></div>
      <p className="modern-performance-hint">{pt ? "O limite vale só para este jogo, mesmo com o painel oculto. Campo vazio usa o limite geral." : "The limit applies only to this game, even with the HUD hidden. Leave blank to use the global limit."}</p>
      <div className="modern-performance-visual"><p>{pt ? "O visual do painel (cores, posição e o que mostrar) é o mesmo para todos os jogos." : "The panel appearance (colors, position and displayed metrics) is shared by all games."}</p><div className="modern-performance-actions"><button type="button" className="modern-mini-btn" onClick={openMonitoringPreferences}>{pt ? "Configurar visual" : "Configure appearance"}</button>{openBenchmarks && <button type="button" className="modern-mini-btn" data-game-benchmark-open onClick={openBenchmarks}>{pt ? "Comparar testes deste jogo" : "Compare this game’s tests"}</button>}</div></div>
      {busy && <p role="status">{pt ? "Salvando…" : "Saving…"}</p>}
      {(error || profileError) && <p role="alert">{error || profileError}</p>}
    </section>
    <section className="modern-performance-card mango-modern modern-performance-preview"><h3>{pt ? "Aparência do monitoramento" : "Monitoring appearance"}</h3>{loaded ? <MonitoringPreview settings={profile}/> : <p role="status">{pt ? "Carregando prévia…" : "Loading preview…"}</p>}</section>
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
