import {mangoSearchOptions} from "./preferenceSearch";
import { SelectionMenu } from "../SelectionMenu";
import { shortcutConflict } from "./shortcutConflicts";
import { notifyError } from "../../services/notifications";
import { MangoHudBenchmarkComparison } from "./MangoHudBenchmarkComparison";
import { MangoHudFpsCycle } from "./MangoHudFpsCycle";
import { MangoHudShortcutRecorder } from "./MangoHudShortcutRecorder";
import { shortcutLabel } from "./mangoHudShortcut";
import { useEffect, useRef, useState, type CSSProperties } from "react";
import { gpuLoadColor } from "./gpuLoadColor";
import catalog from "../../config/mangohudOptions.json";
import { loadMangoHudProfile, saveMangoHudProfile, updateMangoHudPreview, mangoHudPreview, getMangoHudStatus, notifyMangoHudMissing, type MangoHudSettings } from "../../services/mangohud";
import { mangoHudDefaults } from "./mangoHudDefaults";
import "./modernMangoHud.css";

type Spec = { key: string; label: string; section: string; kind: string; default: boolean | number | string; min?: number; max?: number; step?: number; values?: string[] };
const options: Spec[] = catalog;

// Presets put hardware sensors first, followed by FPS and frame timing.
const PRESETS = [
  { id: "basic", name: "Essencial", description: "FPS, CPU, GPU e memória." },
  { id: "full", name: "Detalhado", description: "Sensores, frequências e gráfico." },
  { id: "horizontal", name: "Faixa", description: "Tudo numa linha no topo." },
  { id: "fps", name: "Só FPS", description: "Discreto, só a taxa de quadros." },
];
function applyPreset(kind: string, base: MangoHudSettings): MangoHudSettings {
  const next = { ...base };
  next.launcher_hud_order = "hardware-first";
  for (const option of options) if (option.kind === "toggle" && option.key !== "gpu_load_change" && ["fps", "gpu", "cpu", "extras"].includes(option.section)) next[option.key] = false;
  next.fps = true;
  if (kind !== "fps") for (const key of ["gpu_stats", "cpu_stats", "ram", "vram", "frametime"]) next[key] = true;
  if (kind === "full") for (const key of ["gpu_temp", "cpu_temp", "gpu_core_clock", "gpu_power", "cpu_mhz", "frame_timing", "resolution", "display_server", "wine"]) next[key] = true;
  next.horizontal = kind === "horizontal";
  // Compact MangoHUD removes its native text padding entirely.
  next.hud_compact = false;
  if (kind === "horizontal") next.position = "top-center";
  // Native offsets are signed screen coordinates, so right/bottom anchors
  // need negative values to move inward rather than off screen.
  const [vertical, horizontal] = String(next.position || "top-left").split("-");
  next.offset_x = horizontal === "left" ? 20 : horizontal === "right" ? -20 : 0;
  next.offset_y = vertical === "top" ? 20 : vertical === "bottom" ? -20 : 0;
  if (kind === "fps") { next.fps_metrics = ""; next.custom_text_center = ""; }
  return next;
}
const presetOf = (settings: MangoHudSettings) => PRESETS.find(preset => {
  const expected = applyPreset(preset.id, settings);
  return expected.launcher_hud_order === settings.launcher_hud_order
    && options.every(option => option.kind !== "toggle" || expected[option.key] === settings[option.key]);
})?.id;

// Plain-language metrics; the native keys stay underneath.
const GROUPS: { title: string; color: string; items: [string, string, string?][] }[] = [
  { title: "Desempenho", color: "#34c77b", items: [["fps", "FPS"], ["frametime", "Tempo de quadro"], ["frame_timing", "Gráfico de quadros"], ["fps_metrics", "Média e 1% Low", "avg,0.01"]] },
  { title: "Placa de vídeo", color: "#55b9f3", items: [["gpu_stats", "Uso"], ["gpu_temp", "Temperatura"], ["vram", "Memória (VRAM)"], ["gpu_core_clock", "Frequência"], ["gpu_power", "Consumo"], ["gpu_fan", "Ventoinha"]] },
  { title: "Processador", color: "#b793ef", items: [["cpu_stats", "Uso"], ["cpu_temp", "Temperatura"], ["cpu_mhz", "Frequência"], ["core_load", "Por núcleo"], ["cpu_power", "Consumo"]] },
  { title: "Memória", color: "#dca0d5", items: [["ram", "RAM"], ["swap", "Swap"]] },
  { title: "Sistema", color: "#f4b350", items: [["resolution", "Resolução"], ["time", "Relógio"], ["wine", "Versão do Wine/Proton"], ["gamemode", "GameMode"], ["battery", "Bateria"]] },
];
const PALETTES = [
  { name: "PeliGames", colors: ["FFFFFF", "54C58D", "55B9F3", "DCA0D5", "B793EF"] },
  { name: "Terminal", colors: ["9BEDB0", "41D77A", "67C993", "8FDF9F", "56AC7E"] },
  { name: "Mono", colors: ["FFFFFF", "FFFFFF", "FFFFFF", "FFFFFF", "FFFFFF"] },
  { name: "Âmbar", colors: ["FFE5B0", "F4B350", "FFD787", "E7A25D", "C99568"] },
  { name: "Oceano", colors: ["E6F7FF", "4DD0E1", "64B5F6", "80DEEA", "90CAF9"] },
  { name: "Violeta", colors: ["F5EDFF", "B39DDB", "CE93D8", "D1C4E9", "F0ABFC"] },
];
const POSITIONS: [string, string][] = [["top-left", "Superior esquerdo"], ["top-center", "Topo, centro"], ["top-right", "Superior direito"], ["middle-left", "Meio, esquerda"], ["", ""], ["middle-right", "Meio, direita"], ["bottom-left", "Inferior esquerdo"], ["bottom-center", "Base, centro"], ["bottom-right", "Inferior direito"]];
const LIMITS: [number, string][] = [[0, "Sem limite"], [30, "30"], [60, "60"], [120, "120"], [144, "144"]];
const SECTION_NAMES: Record<string, string> = { fps: "FPS e quadros", visual: "Aparência", gpu: "Placa de vídeo", cpu: "Processador e memória", extras: "Sistema e extras", logs: "Registro de desempenho" };
const SELECT_LABELS: Record<string, Record<string, string>> = { gpu_load_value: { "50,80": "50% / 80%", "60,90": "60% / 90%", "70,95": "70% / 95%" }, gpu_load_color: { "4ADE80,FBBF24,F87171": "Verde, amarelo e vermelho", "64B5F6,CE93D8,F472B6": "Azul, lilás e rosa", "80DEEA,FDE68A,FB923C": "Ciano, amarelo e laranja" }, launcher_hud_order: { "fps-first": "FPS primeiro", "hardware-first": "Hardware primeiro" }, fps_color: { "E06C75,E5C07B,98C379": "Suave", "F87171,FBBF24,4ADE80": "Vermelho, âmbar e verde", "FF5555,FFCC55,55CCFF": "Vermelho, âmbar e azul" }, position: { "top-left": "Superior esquerdo", "top-center": "Superior central", "top-right": "Superior direito", "middle-left": "Centro esquerdo", "middle-right": "Centro direito", "bottom-left": "Inferior esquerdo", "bottom-center": "Inferior central", "bottom-right": "Inferior direito" }, fps_metrics: { "": "Desativado", avg: "Média", "avg,0.01": "Média e 1% Low", "avg,0.01,0.001": "Média, 1% Low e 0.1% Low" }, vsync: { "-1": "Padrão do jogo", "0": "Adaptativo", "1": "Desativado", "2": "Mailbox", "3": "Ativado" }, gl_vsync: { "-2": "Padrão do jogo", "0": "Desativado", "1": "A cada quadro", "2": "A cada 2 quadros", "3": "A cada 3 quadros", "4": "A cada 4 quadros" }, gpu_list: { "": "Seleção automática", "0": "GPU 0", "1": "GPU 1", "0,1": "GPU 0 e GPU 1" }, fps_limit_method: { late: "Após apresentar o quadro", early: "Antes de apresentar o quadro" } };
const keyLabel = shortcutLabel;

/** Simulated overlay drawn from the settings, so every change shows at once. */
export function Hud({ settings, tick, size, graph }: { settings: MangoHudSettings; tick: number; size: number; graph: number[] }) {
  const s = settings, wave = (n: number) => (Math.sin(tick / 3 + n) + 1) / 2;
  const limit = Number(String(s.launcher_fps_limits || s.fps_limit).split(",")[0]) || 0;
  const fps = Math.round(limit > 0 ? Math.min(limit, 141) - wave(1) * 2 : 128 + wave(1) * 14);
  const color = (key: string) => `#${String(s[key] || "FFFFFF")}`;
  const gpuUsage = Math.round(82 + wave(2) * 12);
  const unit = (text: string) => <span className="u">{text}</span>;
  const row = (label: string, tint: string, cells: (React.ReactNode | false)[]) => <div className="r" key={label}><span className="k" style={{ color: tint }}>{label}</span>{cells.filter(Boolean).map((cell, index) => <span key={index}>{cell}</span>)}</div>;
  const rows: React.ReactNode[] = [];
  const performanceRows: React.ReactNode[] = [];
  if (s.engine_version) performanceRows.push(row("VULKAN", color("engine_color"), []));
  if (s.fps) performanceRows.push(row("FPS", color("text_color"), [<span style={{ fontSize: "1.25em" }}>{fps}</span>, Boolean(s.frametime) && <>{(1000 / fps).toFixed(1)}{unit("ms")}</>]));
  if (s.frame_timing) performanceRows.push(<svg className="graph" key="graph" viewBox="0 0 100 40" preserveAspectRatio="none"><polyline points={graph.map((y, i) => `${(i / (graph.length - 1)) * 100},${40 - (y - 10) * 3}`).join(" ")} fill="none" stroke={color("frametime_color")} strokeWidth="2.2" vectorEffect="non-scaling-stroke" /></svg>);
  const metrics = String(s.fps_metrics || "").split(",");
  for (const [key, label, factor] of [["avg", "AVG", .93], ["0.01", "1% Low", .71], ["0.001", "0.1% Low", .59]] as const) {
    if (metrics.includes(key)) performanceRows.push(row(label, color("text_color"), [<>{Math.round(fps * factor)}{unit("FPS")}</>]));
  }
  if (s.custom_text_center) performanceRows.push(<div className="title" key="title">{String(s.custom_text_center)}</div>);
  if (s.gpu_stats || s.gpu_temp || s.gpu_core_clock || s.gpu_power || s.gpu_fan) rows.push(row(String(s.gpu_text || "GPU"), color("gpu_color"), [Boolean(s.gpu_stats) && <span style={s.gpu_load_change ? { color: gpuLoadColor(gpuUsage, String(s.gpu_load_value), String(s.gpu_load_color)) } : undefined}>{gpuUsage}{unit("%")}</span>, Boolean(s.gpu_temp) && <>{Math.round(64 + wave(3) * 4)}{unit("°C")}</>, Boolean(s.gpu_core_clock) && <>{Math.round(2410 + wave(4) * 90)}{unit("MHz")}</>, Boolean(s.gpu_power) && <>{Math.round(212 + wave(5) * 18)}{unit("W")}</>, Boolean(s.gpu_fan) && <>{Math.round(1450 + wave(6) * 120)}{unit("RPM")}</>]));
  if (s.vram) rows.push(row("VRAM", color("vram_color"), [<>{(9.2 + wave(10) * .4).toFixed(1)}{unit("GiB")}</>]));
  if (s.cpu_stats || s.cpu_temp || s.cpu_mhz || s.cpu_power) rows.push(row(String(s.cpu_text || "CPU"), color("cpu_color"), [Boolean(s.cpu_stats) && <>{Math.round(41 + wave(7) * 15)}{unit("%")}</>, Boolean(s.cpu_temp) && <>{Math.round(58 + wave(8) * 5)}{unit("°C")}</>, Boolean(s.cpu_mhz) && <>{Math.round(4780 + wave(9) * 120)}{unit("MHz")}</>, Boolean(s.cpu_power) && <>{Math.round(62 + wave(9) * 9)}{unit("W")}</>]));
  if (s.core_load) rows.push(row("Núcleos", color("cpu_color"), [Array.from({ length: 8 }, (_, i) => Math.round(30 + wave(i * 1.7) * 60)).join(" ")]));
  if (s.ram || s.swap) rows.push(row("RAM", color("ram_color"), [Boolean(s.ram) && <>{(12.4 + wave(11) * .3).toFixed(1)}{unit("GiB")}</>, Boolean(s.swap) && <>0.3{unit("GiB swap")}</>]));
  if (s.launcher_hud_order === "hardware-first") rows.push(...performanceRows);
  else rows.unshift(...performanceRows);
  if (s.resolution) rows.push(row("Tela", color("text_color"), ["2560×1440"]));
  if (s.time) rows.push(row("Hora", color("text_color"), [new Date().toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })]));
  if (s.wine) rows.push(row("Proton", color("text_color"), ["GE-Proton"]));
  if (s.gamemode) rows.push(row("GameMode", color("text_color"), ["ATIVO"]));
  if (s.battery) rows.push(row("Bateria", color("text_color"), [<>87{unit("%")}</>]));
  if (!rows.length) rows.push(<div className="title" key="empty">Nada selecionado</div>);
  const background = String(s.background_color || "020202").match(/../g)?.map(hex => parseInt(hex, 16)).join(",") ?? "2,2,2";
  const [vertical, horizontal] = String(s.position || "top-left").split("-");
  const scale = size / Number(s.font_size || 24);
  const offsetX = Number(s.offset_x || 0) * scale, offsetY = Number(s.offset_y || 0) * scale;
  const pad = (Number(s.offset_x) > 0 || Number(s.offset_y) > 0 ? 0 : 10) * scale;
  const place: CSSProperties = {};
  const shift: string[] = [];
  if (vertical === "top") place.top = pad + offsetY; else if (vertical === "bottom") place.bottom = pad - offsetY; else { place.top = `calc(50% + ${offsetY}px)`; shift.push("translateY(-50%)"); }
  if (horizontal === "left") place.left = pad + offsetX; else if (horizontal === "right") place.right = pad - offsetX; else { place.left = "50%"; shift.push("translateX(-50%)"); }
  return <div className={`mango-hud${s.horizontal ? " horizontal" : ""}${s.hud_compact ? " compact" : ""}${s.text_outline ? " outline" : ""}`}
    style={{ ...place, transform: shift.join(" ") || undefined, fontSize: `${size}px`, "--hud-text": color("text_color"), "--hud-bg": background, "--hud-alpha": String(s.background_alpha), "--hud-radius": `${Number(s.round_corners) * .06}em` } as CSSProperties}>
    <div className="hud-box">{rows}</div>
  </div>;
}

const Check = () => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"><path d="m5 12 5 5 9-10" /></svg>;

/** New interface: the MangoHUD profile in three plain steps beside a live
 *  preview, with every native option still reachable in "Todas as opções".
 *  Loads and saves the same profile as the classic editor. */
export function ModernMangoHudEditor({ onDirtyChange, searchQuery = "", visible=true, benchmarkOnly=false }: { onDirtyChange: (dirty: boolean) => void; searchQuery?:string; visible?:boolean; benchmarkOnly?:boolean }) {
  const searching=Boolean(searchQuery.trim());
  const [settings, setSettings] = useState<MangoHudSettings>(mangoHudDefaults), [saved, setSaved] = useState("");
  const [ready, setReady] = useState(false), [saving, setSaving] = useState(false), [error, setError] = useState(""), [saveError, setSaveError] = useState("");
  const [previewBusy, setPreviewBusy] = useState(false), [running, setRunning] = useState(false);
  const [checkingGlobal, setCheckingGlobal] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false), [query, setQuery] = useState("");
  const [tick, setTick] = useState(0), [graph, setGraph] = useState(() => Array.from({ length: 48 }, () => 16 + Math.random() * 3));
  const [screenWidth, setScreenWidth] = useState(480);
  const screen = useRef<HTMLDivElement>(null);
  const mounted = useRef(false), saveLock = useRef(false), previewLock = useRef(false), globalLock = useRef(false), previewQueue = useRef(Promise.resolve());
  const readyRef = useRef(ready), savedRef = useRef(saved), latest = useRef(settings);
  readyRef.current = ready; savedRef.current = saved; latest.current = settings;
  const dirty = ready && JSON.stringify(settings) !== saved;
  useEffect(() => { onDirtyChange(dirty || saving || previewBusy || checkingGlobal); return () => onDirtyChange(false); }, [dirty, saving, previewBusy, checkingGlobal, onDirtyChange]);
  const load = async () => {
    setError("");
    try { const profile = await loadMangoHudProfile(); if (mounted.current) { const next = { ...mangoHudDefaults(), ...profile.settings }; setSettings(next); setSaved(JSON.stringify(next)); setReady(true); } }
    catch (e) { if (mounted.current) setError(String(e)); }
  };
  useEffect(() => {
    mounted.current = true; void load();
    return () => {
      mounted.current = false;
      if (readyRef.current && JSON.stringify(latest.current) !== savedRef.current) void saveMangoHudProfile(latest.current).catch(() => {});
      void mangoHudPreview("stop").catch(() => {});
    };
  }, []);
  const save = async () => {
    if (saveLock.current) return;
    saveLock.current = true; setSaving(true); setSaveError("");
    const snapshot = latest.current;
    try { await saveMangoHudProfile(snapshot); if (mounted.current) setSaved(JSON.stringify(snapshot)); }
    catch (e) { if (mounted.current) setSaveError(e instanceof Error ? e.message : String(e)); }
    finally { saveLock.current = false; if (mounted.current) setSaving(false); }
  };
  useEffect(() => {
    if (!ready || !dirty || saving || saveError) return;
    const timer = setTimeout(() => void save(), 500);
    return () => clearTimeout(timer);
  }, [settings, ready, dirty, saving, saveError]); // eslint-disable-line react-hooks/exhaustive-deps
  // The real MangoHUD window, when opened, follows every change.
  useEffect(() => {
    if (!running) return;
    const timer = setTimeout(() => { const snapshot = settings; previewQueue.current = previewQueue.current.catch(() => {}).then(async () => { if (mounted.current) await updateMangoHudPreview(snapshot); }).catch(e => { if (mounted.current) setError(String(e)); }); }, 180);
    return () => clearTimeout(timer);
  }, [settings, running]);
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => { void mangoHudPreview("status").then(status => { if (mounted.current && !status.running) setRunning(false); }).catch(e => { if (mounted.current) { setError(String(e)); setRunning(false); } }); }, 1500);
    return () => clearInterval(timer);
  }, [running]);
  useEffect(() => {
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduced || searching || !visible) return;
    const timer = setInterval(() => { setTick(value => value + 1); setGraph(points => [...points.slice(1), 16 + Math.random() * 3 + (Math.random() < .06 ? 6 : 0)]); }, 450);
    return () => clearInterval(timer);
  }, [searching,visible]);
  useEffect(() => {
    const element = screen.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => setScreenWidth(entry.contentRect.width));
    observer.observe(element);
    return () => observer.disconnect();
  }, [ready,searching,visible]);
  const update = (patch: MangoHudSettings) => {
    const next = { ...latest.current, ...patch };
    for (const option of options) {
      if (option.kind !== "shortcut" || !(option.key in patch) || patch[option.key] === latest.current[option.key]) continue;
      const conflict = shortcutConflict(option.key, String(patch[option.key]), next);
      if (conflict) {
        notifyError("Atalho em uso", `Essa combinação já está atribuída a “${conflict}”. Escolha outro atalho. O atalho anterior foi mantido.`);
        return;
      }
    }
    setSettings(current => ({ ...current, ...patch })); setSaveError("");
  };
  const set = (key: string, value: boolean | number | string) => update({ [key]: value });
  const toggleGlobal = async () => {
    if (globalLock.current) return;
    if (settings.launcher_enabled) { set("launcher_enabled", false); return; }
    globalLock.current = true; setCheckingGlobal(true); setError("");
    try { const status = await getMangoHudStatus(); if (!mounted.current) return; if (status.installed) set("launcher_enabled", true); else { notifyMangoHudMissing(); } }
    catch (e) { if (mounted.current) setError(e instanceof Error ? e.message : String(e)); }
    finally { globalLock.current = false; if (mounted.current) setCheckingGlobal(false); }
  };
  const preview = async () => {
    if (previewLock.current) return;
    previewLock.current = true; setPreviewBusy(true); setError("");
    try { await previewQueue.current; const status = await mangoHudPreview(running ? "stop" : "start", latest.current); if (mounted.current) setRunning(status.running); }
    catch (e) { if (mounted.current) setError(String(e)); }
    finally { previewLock.current = false; if (mounted.current) setPreviewBusy(false); }
  };
  const current = presetOf(settings);
  const limit = Number(settings.fps_limit) || 0;
  const customLimit = limit > 0 && !LIMITS.some(([value]) => value === limit);
  const layout = settings.horizontal ? "horizontal" : settings.hud_compact ? "compact" : "vertical";
  const slider = (key: string, min: number, max: number, step: number, format: (value: number) => string, label: string) => {
    const value = Number(settings[key]);
    return <><input type="range" aria-label={label} min={min} max={max} step={step} value={value} style={{ "--p": `${((value - min) / (max - min)) * 100}%` } as CSSProperties} onChange={event => set(key, Number(event.target.value))} /><span className="value">{format(value)}</span></>;
  };
  const filtered = options.filter(option => option.section !== "launcher" && (!query.trim() || option.label.toLowerCase().includes(query.trim().toLowerCase()) || option.key.includes(query.trim().toLowerCase())));
  const sections = [...new Set(filtered.map(option => option.section))];
  const advancedControl = (option: Spec) => {
    const value = settings[option.key];
    if(option.key==="launcher_enabled")return <button type="button" className="mango-switch sm" role="switch" aria-label={option.label} aria-checked={Boolean(value)} disabled={checkingGlobal} onClick={()=>void toggleGlobal()}/>;
    if(option.kind==="fpslimits")return <MangoHudFpsCycle value={String(value||"")} onChange={value=>set(option.key,value)}/>;
    if (option.kind === "toggle") return <button type="button" className="mango-switch sm" role="switch" aria-checked={Boolean(value)} aria-label={option.label} onClick={() => set(option.key, !value)} />;
    if (option.kind === "shortcut") return <MangoHudShortcutRecorder label={option.label} value={String(value)} onChange={shortcut => set(option.key, shortcut)} />;
    if (option.kind === "select") return <SelectionMenu label={option.label} className="mm-setting-select" menuClassName="mm-setting-options" value={String(value)} onChange={value => set(option.key, value)} options={(option.values ?? []).map(item => ({value:item,label:SELECT_LABELS[option.key]?.[item] ?? (item === "" ? "(desligado)" : item)}))} />;
    if (option.kind === "color") return <input type="color" aria-label={option.label} value={`#${String(value)}`} onChange={event => set(option.key, event.target.value.slice(1).toUpperCase())} />;
    if (option.kind === "text") return <input type="text" aria-label={option.label} maxLength={64} value={String(value ?? "")} onChange={event => set(option.key, event.target.value.replace(/[\x00-\x1f\x7f]/g, ""))} />;
    return <input type="number" aria-label={option.label} min={option.min} max={option.max} step={option.step} value={Number(value)} onChange={event => { const number = event.target.valueAsNumber; if (Number.isFinite(number)) { const clamped=Math.min(option.max ?? 1000, Math.max(option.min ?? 0, number)); if(option.key==="fps_limit")update({fps_limit:clamped,launcher_fps_limits:""});else set(option.key,clamped); } }} />;
  };
  const status = saveError ? "Falha ao salvar" : saving ? "Salvando…" : dirty ? "Aguardando salvamento…" : "Salvo automaticamente";

  if(searching){
    const matches=mangoSearchOptions(searchQuery);if(!matches.length)return null;const groups=[...new Set(matches.map(option=>option.section))];
    return <section className="preference-search-group mango-modern mango-search-results" aria-label="MangoHUD"><header><h2>MangoHUD</h2><span className="mm-saved" role="status">{ready?status:"Carregando configuração…"}</span></header>
      {error&&<p role="alert" className="preferences-error">{error}<button type="button" onClick={()=>void load()}>Tentar novamente</button></p>}
      {saveError&&<div role="alert" className="preferences-error"><p>{saveError}</p><button type="button" disabled={saving} onClick={()=>setSaveError("")}>Tentar novamente</button><button type="button" onClick={()=>{setSettings(JSON.parse(saved));setSaveError("")}}>Desfazer alterações</button></div>}
      {ready&&groups.map(section=><div key={section} className="mm-advanced-section"><h3>{SECTION_NAMES[section]||"Integração com o launcher"}</h3>{matches.filter(option=>option.section===section).map(option=><div key={option.key} className="mm-advanced-row"><span>{option.label}{option.key==="fps_limit"&&<small>0 = sem limite. Alterar este campo substitui a lista de limites.</small>}</span>{advancedControl(option)}</div>)}</div>)}
    </section>;
  }
  if (!ready) return <div className="mango-modern"><section className="mm-card"><p>{error || "Carregando configuração…"}</p>{error && <button type="button" className="mm-ghost" onClick={() => void load()}>Tentar novamente</button>}</section></div>;
  return <div className="mango-modern">
    <div className="monitoring-benchmark-screen" hidden={!benchmarkOnly}><MangoHudBenchmarkComparison shortcut={keyLabel(String(settings.toggle_logging))} fullPage /></div>
    <div className="monitoring-editor-screen" hidden={benchmarkOnly}>
    <div className="mm-head">
      <div className="mm-head-copy"><h2>{advancedOpen?"Configuração avançada":"Configuração simples"}</h2><p>{advancedOpen?"Todas as opções do MangoHUD, organizadas por categoria.":"Estilos prontos e ajustes essenciais para o seu painel."}</p></div>
      <div className="mm-head-controls"><div className="mm-segmented" role="tablist" aria-label="Modo de configuração" onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;event.preventDefault();const next=event.key==="Home"?false:event.key==="End"?true:!advancedOpen;setAdvancedOpen(next);requestAnimationFrame(()=>document.getElementById(next?"mango-mode-advanced":"mango-mode-simple")?.focus())}}><button type="button" role="tab" id="mango-mode-simple" aria-controls="mango-mode-panel" aria-selected={!advancedOpen} tabIndex={advancedOpen?-1:0} onClick={() => setAdvancedOpen(false)}>Simples</button><button type="button" role="tab" id="mango-mode-advanced" aria-controls="mango-mode-panel" aria-selected={advancedOpen} tabIndex={advancedOpen?0:-1} onClick={() => setAdvancedOpen(true)}>Avançado</button></div>
      <span className={`mm-saved${saveError ? " failed" : dirty || saving ? " pending" : ""}`} role="status"><i />{status}</span></div>
    </div>
    {saveError && <div role="alert" className="preferences-error"><p>Não foi possível salvar automaticamente: {saveError}</p><div className="monitoring-actions"><button type="button" disabled={saving} onClick={() => setSaveError("")}>Tentar novamente</button><button type="button" disabled={saving} onClick={() => { setSettings(JSON.parse(saved)); setSaveError(""); }}>Desfazer alterações</button></div></div>}
    {error && <p role="alert" className="preferences-error">{error}</p>}
    <div className="mm-grid">
      <div className="mm-stack" id="mango-mode-panel" role="tabpanel" aria-labelledby={advancedOpen?"mango-mode-advanced":"mango-mode-simple"}>
        <section className="mm-card mm-master">
          <div className="mm-big-icon"><svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2"><path d="M3 12h4l3-8 4 16 3-8h4" /></svg></div>
          <div><strong>Mostrar nos jogos</strong><small>Em todos os jogos abertos pelo PeliGames. No jogo, <kbd>{keyLabel(String(settings.toggle_hud || "Shift_R+F12"))}</kbd> mostra ou esconde.</small>
            {checkingGlobal && <small role="status">Verificando MangoHUD…</small>}
          </div>
          <div className="mm-spacer" />
          <button type="button" className="mango-switch" role="switch" aria-checked={Boolean(settings.launcher_enabled)} aria-label="Mostrar nos jogos" disabled={checkingGlobal} onClick={() => void toggleGlobal()} />
        </section>

        <section className="mm-card mm-system-sync">
          <div><strong>Aplicar também ao MangoHUD do sistema</strong><p>Atualiza a configuração do seu usuário para jogos fora do launcher. A configuração anterior fica preservada em backup.</p></div>
          <button type="button" className="mango-switch" role="switch" aria-label="Aplicar também ao MangoHUD do sistema" aria-checked={Boolean(settings.launcher_sync_system)} onClick={()=>set("launcher_sync_system",!settings.launcher_sync_system)} />
        </section>

        {advancedOpen?<section className="mm-card mm-advanced-screen" aria-label="Todas as opções do MangoHUD"><h2>Todas as opções</h2><p className="mm-hint">Ajustes completos. Use a busca para localizar uma opção específica.</p>          <div className="mm-advanced-body">
            <label className="mm-search"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg><input value={query} onChange={event => setQuery(event.target.value)} placeholder="Buscar opção (ex.: vsync, bateria, ventoinha)" /></label>
            {sections.map(section => <div key={section} className="mm-advanced-section"><h4>{SECTION_NAMES[section] ?? section}</h4>
              {filtered.filter(option => option.section === section).map(option => <div key={option.key} className="mm-advanced-row"><span>{option.label}</span>{advancedControl(option)}</div>)}
            </div>)}
            {!sections.length && <p className="mm-empty">Nenhuma opção com “{query}”.</p>}
          </div>
</section>:<>
        <section className="mm-card">
          <h2><span className="mm-step">1</span>Escolha um estilo</h2>
          <p className="mm-hint">Um ponto de partida pronto. Depois ajuste o que quiser nos passos abaixo.</p>
          <div className="mm-presets">{PRESETS.map(preset => {
            const example = { ...applyPreset(preset.id, settings), background_alpha: settings.background_alpha };
            return <button type="button" key={preset.id} className="mm-preset" aria-pressed={current === preset.id} onClick={() => update(applyPreset(preset.id, settings))}>
              <div className="mm-thumb" aria-hidden="true"><Hud settings={example} tick={tick} size={6.5} graph={graph} /></div>
              <strong>{preset.name}</strong><small>{preset.description}</small>
            </button>;
          })}</div>
        </section>

        <section className="mm-card">
          <h2><span className="mm-step">2</span>O que mostrar</h2>
          <p className="mm-hint">Toque para ligar ou desligar. A prévia ao lado muda na hora.</p>
          <div className="mm-groups">{GROUPS.map(group => <div key={group.title} className="mm-group">
            <h3><i style={{ background: group.color }} />{group.title}</h3>
            <div className="mm-chips">{group.items.filter(([key]) => key in settings).map(([key, label, on]) => {
              const active = key === "fps_metrics" ? String(settings[key] || "").split(",").includes("0.01") : on ? settings[key] === on : Boolean(settings[key]);
              return <button type="button" key={key} className="mm-chip" aria-pressed={active} onClick={() => set(key, on ? (active ? "" : on) : !settings[key])}><Check />{label}</button>;
            })}</div>
          </div>)}</div>
        </section>

        <section className="mm-card">
          <h2><span className="mm-step">3</span>Aparência</h2>
          <p className="mm-hint">Para mudar a posição, clique no lugar desejado na prévia.</p>
          <div className="mm-rows">
            <div className="mm-row"><span>Formato</span><div className="mm-segmented">{([["vertical", "Coluna"], ["horizontal", "Linha"], ["compact", "Compacto"]] as const).map(([id, label]) => <button type="button" key={id} aria-pressed={layout === id}
              onClick={() => update({ horizontal: id === "horizontal", hud_compact: id === "compact", ...(id === "horizontal" && String(settings.position).startsWith("middle") ? { position: "top-center" } : {}) })}>{label}</button>)}</div></div>
            <div className="mm-row"><span>Ordem das informações<small>GPU, VRAM, CPU e RAM antes ou depois do FPS</small></span><div className="mm-segmented" role="group" aria-label="Ordem das informações">
              {([["fps-first", "FPS primeiro"], ["hardware-first", "Hardware primeiro"]] as const).map(([value, label]) => <button type="button" key={value} aria-pressed={settings.launcher_hud_order === value} onClick={() => set("launcher_hud_order", value)}>{label}</button>)}
            </div></div>
            <div className="mm-row"><span>Tamanho do texto</span>{slider("font_size", 14, 40, 1, value => `${value}px`, "Tamanho do texto")}</div>
            <div className="mm-row"><span>Fundo<small>Quanto o painel esconde o jogo</small></span>{slider("background_alpha", 0, 1, .05, value => `${Math.round(value * 100)}%`, "Fundo")}</div>
            <div className="mm-row"><span>Cantos arredondados</span>{slider("round_corners", 0, 20, 1, value => String(value), "Cantos arredondados")}</div>
            <div className="mm-row"><span>Cores</span><div className="mm-palettes">{PALETTES.map(palette => <button type="button" key={palette.name} className="mm-palette"
              aria-pressed={palette.colors[0] === settings.text_color && palette.colors[1] === settings.gpu_color && palette.colors[2] === settings.cpu_color}
              onClick={() => update({ text_color: palette.colors[0], gpu_color: palette.colors[1], cpu_color: palette.colors[2], ram_color: palette.colors[3], vram_color: palette.colors[4], frametime_color: palette.colors[1] })}>
              <span aria-hidden="true">{palette.colors.slice(1).map((color, index) => <i key={index} style={{ background: `#${color}` }} />)}</span>{palette.name}</button>)}</div></div>
            <div className="mm-row"><span>Cor pelo uso da GPU<small>Muda a cor do percentual conforme a carga</small></span><button type="button" className="mango-switch sm" role="switch" aria-label="Cor pelo uso da GPU" aria-checked={Boolean(settings.gpu_load_change)} onClick={() => set("gpu_load_change", !settings.gpu_load_change)} /></div>
            <div className="mm-row"><span>Faixas de uso da GPU<small>Limites de carga média e alta</small></span><SelectionMenu label="Faixas de uso da GPU" className="mm-setting-select" menuClassName="mm-setting-options" disabled={!settings.gpu_load_change} value={String(settings.gpu_load_value)} onChange={value => set("gpu_load_value", value)} options={(options.find(option => option.key === "gpu_load_value")?.values ?? []).map(value => ({value,label:SELECT_LABELS.gpu_load_value[value]}))} /></div>
            <div className="mm-row"><span>Cores por uso da GPU<small>Carga baixa, média e alta</small></span><SelectionMenu label="Cores por uso da GPU" className="mm-setting-select" menuClassName="mm-setting-options" disabled={!settings.gpu_load_change} value={String(settings.gpu_load_color)} onChange={value => set("gpu_load_color", value)} options={(options.find(option => option.key === "gpu_load_color")?.values ?? []).map(value => ({value,label:SELECT_LABELS.gpu_load_color[value]}))} /></div>
            <div className="mm-row"><span>Contorno no texto<small>Ajuda a ler em cenas claras</small></span><button type="button" className="mango-switch sm" role="switch" aria-label="Contorno no texto" aria-checked={Boolean(settings.text_outline)} onClick={() => set("text_outline", !settings.text_outline)} /></div>
          </div>
        </section>

        <section className="mm-card">
          <h2><span className="mm-step">⚡</span>Limitar FPS</h2>
          <p className="mm-hint">Menos FPS = menos calor, ruído e consumo de bateria. Útil em jogos leves.</p>
          <div className="mm-limit">
            <div className="mm-segmented">{LIMITS.map(([value, label]) => <button type="button" key={value} aria-pressed={!settings.launcher_fps_limits && !customLimit && limit === value} onClick={() => update({ fps_limit: value, launcher_fps_limits: "" })}>{label}</button>)}
              <button type="button" aria-pressed={!settings.launcher_fps_limits && customLimit} onClick={() => update({ fps_limit: customLimit ? limit : 90, launcher_fps_limits: "" })}>Outro</button></div>
            {customLimit && <input className="mm-limit-input" type="number" min={1} max={1000} aria-label="Limite personalizado" value={limit} onChange={event => { const number = event.target.valueAsNumber; if (Number.isFinite(number)) update({ fps_limit: Math.max(1, Math.min(1000, Math.round(number))), launcher_fps_limits: "" }); }} />}
          </div>
          <div className="mm-rows">
            <div className="mm-row"><span>Alternar limites durante o jogo<small>Use 0 para sem limite; a lista substitui o limite único.</small></span><MangoHudFpsCycle value={String(settings.launcher_fps_limits || "")} onChange={value=>set("launcher_fps_limits",value)} /></div>
            <div className="mm-segmented">{["30,60,120", "60,120,144", "30,60,0"].map(value=><button key={value} type="button" aria-pressed={settings.launcher_fps_limits===value} onClick={()=>set("launcher_fps_limits",value)}>{value.split(",").map(v=>v==="0"?"Sem limite":v).join(" → ")}</button>)}</div>
            <div className="mm-row"><span>Atalho para alternar FPS</span><MangoHudShortcutRecorder label="Atalho para alternar limites de FPS" value={String(settings.toggle_fps_limit)} onChange={value=>set("toggle_fps_limit",value)} /></div>
          </div>
          <p className="mm-hint">Um limite configurado nas opções de um jogo tem prioridade sobre esta lista.</p>
        </section>

        <section className="mm-card">
          <h2>Atalhos no jogo</h2>
          <p className="mm-hint">Clique no atalho e pressione a tecla ou combinação desejada. Pode usar Ctrl, Alt e Shift esquerdo ou direito. Esc cancela a captura.</p>
          <div className="mm-rows">{([
            ["toggle_hud", "Mostrar ou ocultar painel", "Exibe ou esconde o MangoHUD.", "Shift_R+F12"],
            ["toggle_logging", "Iniciar ou encerrar gravação", "Cada gravação gera um teste separado.", "Shift_L+F2"],
            ["reset_fps_metrics", "Redefinir médias", "Zera as médias e mínimos exibidos no painel; não apaga os registros CSV.", "Shift_R+F9"],
          ] as const).map(([key,label,hint,defaultKey])=><div className="mm-row" key={key}>
            <span>{label}<small>{hint}</small></span>
            <div className="mm-shortcut-actions"><MangoHudShortcutRecorder label={label} value={String(settings[key] || defaultKey)} onChange={value=>set(key,value)} />
              <button type="button" className="mm-ghost" aria-label={`Restaurar atalho padrão: ${label}`} title={`Padrão: ${keyLabel(defaultKey)}`} disabled={settings[key]===defaultKey} onClick={()=>set(key,defaultKey)}>Padrão</button>
            </div>
          </div>)}</div>
        </section>



        </>}
      </div>

      <div className="mm-preview-column">
      <aside className="mm-card mm-preview" aria-label="Prévia">
        <div className="mm-preview-head"><strong>Prévia</strong><small>— passe o mouse e clique para posicionar</small></div>
        <div ref={screen} className="mm-screen">
          {/* Always drawn: games with MangoHUD turned on individually use this profile too. */}
          <Hud settings={settings} tick={tick} size={Number(settings.font_size) * (screenWidth / 2560) * 2.6} graph={graph} />
          <div className="mm-zones">{POSITIONS.map(([id, label], index) => id ? <button type="button" key={id} aria-label={label} aria-pressed={settings.position === id} onClick={() => set("position", id)}>{label}</button> : <span key={index} />)}</div>
        </div>
        <div className="mm-preview-foot">
          <span>Posição: {(POSITIONS.find(([id]) => id === settings.position) ?? ["", "Superior esquerdo"])[1]}</span><span className="mm-spacer" />
          <button type="button" className="mm-ghost" disabled={previewBusy} onClick={() => void preview()} title="Abre o MangoHUD de verdade numa janela de teste">{previewBusy ? "Aguarde…" : running ? "Fechar teste" : "Testar no sistema"}</button>
          <button type="button" className="mm-ghost" onClick={() => update({ ...mangoHudDefaults(), launcher_enabled: settings.launcher_enabled, launcher_sync_system: settings.launcher_sync_system })}>Restaurar padrão</button>
        </div>
        {running && <small className="mm-running">Janela de teste aberta · atualização ao vivo</small>}
      </aside>
      </div>
    </div>
    </div>
  </div>;
}
