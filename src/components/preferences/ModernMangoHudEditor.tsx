import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import catalog from "../../config/mangohudOptions.json";
import { loadMangoHudProfile, saveMangoHudProfile, updateMangoHudPreview, mangoHudPreview, getMangoHudStatus, type MangoHudSettings } from "../../services/mangohud";
import { mangoHudDefaults } from "./mangoHudDefaults";
import "./modernMangoHud.css";

type Spec = { key: string; label: string; section: string; kind: string; default: boolean | number | string; min?: number; max?: number; step?: number; values?: string[] };
const options: Spec[] = catalog;

// Same starting points as the classic editor (MangoHudEditor preset()).
const PRESETS = [
  { id: "basic", name: "Essencial", description: "FPS, CPU, GPU e memória." },
  { id: "full", name: "Detalhado", description: "Sensores, frequências e gráfico." },
  { id: "horizontal", name: "Faixa", description: "Tudo numa linha no topo." },
  { id: "fps", name: "Só FPS", description: "Discreto, só a taxa de quadros." },
];
function applyPreset(kind: string, base: MangoHudSettings): MangoHudSettings {
  const next = { ...base };
  for (const option of options) if (option.kind === "toggle" && ["fps", "gpu", "cpu", "extras"].includes(option.section)) next[option.key] = false;
  next.fps = true;
  if (kind !== "fps") for (const key of ["gpu_stats", "cpu_stats", "ram", "vram", "frametime"]) next[key] = true;
  if (kind === "full") for (const key of ["gpu_temp", "cpu_temp", "gpu_core_clock", "gpu_power", "cpu_mhz", "frame_timing", "resolution", "display_server", "wine"]) next[key] = true;
  next.horizontal = kind === "horizontal";
  if (kind === "horizontal") next.position = "top-center";
  if (kind === "fps") { next.fps_metrics = ""; next.custom_text_center = ""; next.hud_compact = true; }
  return next;
}
const presetOf = (settings: MangoHudSettings) => PRESETS.find(preset => {
  const expected = applyPreset(preset.id, settings);
  return options.every(option => option.kind !== "toggle" || expected[option.key] === settings[option.key]);
})?.id;

// Plain-language metrics; the native keys stay underneath.
const GROUPS: { title: string; color: string; items: [string, string, string?][] }[] = [
  { title: "Desempenho", color: "#34c77b", items: [["fps", "FPS"], ["frametime", "Tempo de quadro"], ["frame_timing", "Gráfico de quadros"], ["fps_metrics", "Média e 1% mínimo", "avg,0.01"]] },
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
];
const POSITIONS: [string, string][] = [["top-left", "Superior esquerdo"], ["top-center", "Topo, centro"], ["top-right", "Superior direito"], ["middle-left", "Meio, esquerda"], ["", ""], ["middle-right", "Meio, direita"], ["bottom-left", "Inferior esquerdo"], ["bottom-center", "Base, centro"], ["bottom-right", "Inferior direito"]];
const LIMITS: [number, string][] = [[0, "Sem limite"], [30, "30"], [60, "60"], [120, "120"], [144, "144"]];
const SECTION_NAMES: Record<string, string> = { fps: "FPS e quadros", visual: "Aparência", gpu: "Placa de vídeo", cpu: "Processador e memória", extras: "Sistema e extras", logs: "Registro de desempenho" };
const SELECT_LABELS: Record<string, Record<string, string>> = { fps_color: { "E06C75,E5C07B,98C379": "Suave", "F87171,FBBF24,4ADE80": "Vermelho, âmbar e verde", "FF5555,FFCC55,55CCFF": "Vermelho, âmbar e azul" }, position: { "top-left": "Superior esquerdo", "top-center": "Superior central", "top-right": "Superior direito", "middle-left": "Centro esquerdo", "middle-right": "Centro direito", "bottom-left": "Inferior esquerdo", "bottom-center": "Inferior central", "bottom-right": "Inferior direito" }, fps_metrics: { "": "Desativado", avg: "Média", "avg,0.01": "Média e 1% mínimo", "avg,0.01,0.001": "Média, 1% e 0,1% mínimo" }, vsync: { "-1": "Padrão do jogo", "0": "Adaptativo", "1": "Desativado", "2": "Mailbox", "3": "Ativado" }, gl_vsync: { "-2": "Padrão do jogo", "0": "Desativado", "1": "A cada quadro", "2": "A cada 2 quadros", "3": "A cada 3 quadros", "4": "A cada 4 quadros" }, gpu_list: { "": "Seleção automática", "0": "GPU 0", "1": "GPU 1", "0,1": "GPU 0 e GPU 1" }, fps_limit_method: { late: "Após apresentar o quadro", early: "Antes de apresentar o quadro" } };
const keyLabel = (value: string) => value.replace("Shift_R", "Shift Direito").replace("Shift_L", "Shift Esquerdo").split("+").join(" + ");

/** Simulated overlay drawn from the settings, so every change shows at once. */
export function Hud({ settings, tick, size, graph }: { settings: MangoHudSettings; tick: number; size: number; graph: number[] }) {
  const s = settings, wave = (n: number) => (Math.sin(tick / 3 + n) + 1) / 2;
  const limit = Number(s.fps_limit) || 0;
  const fps = Math.round(limit > 0 ? Math.min(limit, 141) - wave(1) * 2 : 128 + wave(1) * 14);
  const color = (key: string) => `#${String(s[key] || "FFFFFF")}`;
  const unit = (text: string) => <span className="u">{text}</span>;
  const row = (label: string, tint: string, cells: (React.ReactNode | false)[]) => <div className="r" key={label}><span className="k" style={{ color: tint }}>{label}</span>{cells.filter(Boolean).map((cell, index) => <span key={index}>{cell}</span>)}</div>;
  const rows: React.ReactNode[] = [];
  if (s.custom_text_center) rows.push(<div className="title" key="title">{String(s.custom_text_center)}</div>);
  if (s.gpu_stats || s.gpu_temp || s.gpu_core_clock || s.gpu_power || s.gpu_fan) rows.push(row(String(s.gpu_text || "GPU"), color("gpu_color"), [Boolean(s.gpu_stats) && <>{Math.round(82 + wave(2) * 12)}{unit("%")}</>, Boolean(s.gpu_temp) && <>{Math.round(64 + wave(3) * 4)}{unit("°C")}</>, Boolean(s.gpu_core_clock) && <>{Math.round(2410 + wave(4) * 90)}{unit("MHz")}</>, Boolean(s.gpu_power) && <>{Math.round(212 + wave(5) * 18)}{unit("W")}</>, Boolean(s.gpu_fan) && <>{Math.round(1450 + wave(6) * 120)}{unit("RPM")}</>]));
  if (s.cpu_stats || s.cpu_temp || s.cpu_mhz || s.cpu_power) rows.push(row(String(s.cpu_text || "CPU"), color("cpu_color"), [Boolean(s.cpu_stats) && <>{Math.round(41 + wave(7) * 15)}{unit("%")}</>, Boolean(s.cpu_temp) && <>{Math.round(58 + wave(8) * 5)}{unit("°C")}</>, Boolean(s.cpu_mhz) && <>{Math.round(4780 + wave(9) * 120)}{unit("MHz")}</>, Boolean(s.cpu_power) && <>{Math.round(62 + wave(9) * 9)}{unit("W")}</>]));
  if (s.core_load) rows.push(row("Núcleos", color("cpu_color"), [Array.from({ length: 8 }, (_, i) => Math.round(30 + wave(i * 1.7) * 60)).join(" ")]));
  if (s.vram) rows.push(row("VRAM", color("vram_color"), [<>{(9.2 + wave(10) * .4).toFixed(1)}{unit("GiB")}</>]));
  if (s.ram || s.swap) rows.push(row("RAM", color("ram_color"), [Boolean(s.ram) && <>{(12.4 + wave(11) * .3).toFixed(1)}{unit("GiB")}</>, Boolean(s.swap) && <>0.3{unit("GiB swap")}</>]));
  if (s.fps) rows.push(row("FPS", color("text_color"), [<span style={{ fontSize: "1.25em" }}>{fps}</span>, Boolean(s.frametime) && <>{(1000 / fps).toFixed(1)}{unit("ms")}</>, Boolean(s.fps_metrics) && <>{Math.round(fps * .93)}{unit("avg")} {Math.round(fps * .71)}{unit("1%")}</>]));
  if (s.frame_timing) rows.push(<svg className="graph" key="graph" viewBox="0 0 100 40" preserveAspectRatio="none"><polyline points={graph.map((y, i) => `${(i / (graph.length - 1)) * 100},${40 - (y - 10) * 3}`).join(" ")} fill="none" stroke={color("frametime_color")} strokeWidth="2.2" vectorEffect="non-scaling-stroke" /></svg>);
  if (s.resolution) rows.push(row("Tela", color("text_color"), ["2560×1440"]));
  if (s.time) rows.push(row("Hora", color("text_color"), [new Date().toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })]));
  if (s.wine) rows.push(row("Proton", color("text_color"), ["GE-Proton"]));
  if (s.gamemode) rows.push(row("GameMode", color("text_color"), ["ATIVO"]));
  if (s.battery) rows.push(row("Bateria", color("text_color"), [<>87{unit("%")}</>]));
  if (!rows.length) rows.push(<div className="title" key="empty">Nada selecionado</div>);
  const background = String(s.background_color || "020202").match(/../g)?.map(hex => parseInt(hex, 16)).join(",") ?? "2,2,2";
  const [vertical, horizontal] = String(s.position || "top-left").split("-");
  const pad = s.horizontal && s.horizontal_stretch ? "0" : "3%";
  const place: CSSProperties = {};
  const shift: string[] = [];
  if (vertical === "top") place.top = pad; else if (vertical === "bottom") place.bottom = pad; else { place.top = "50%"; shift.push("translateY(-50%)"); }
  if (horizontal === "left") place.left = pad; else if (horizontal === "right") place.right = pad; else { place.left = "50%"; shift.push("translateX(-50%)"); }
  return <div className={`mango-hud${s.horizontal ? " horizontal" : ""}${s.hud_compact ? " compact" : ""}${s.text_outline ? " outline" : ""}`}
    style={{ ...place, transform: shift.join(" ") || undefined, fontSize: `${size}px`, "--hud-text": color("text_color"), "--hud-bg": background, "--hud-alpha": String(s.background_alpha), "--hud-radius": `${Number(s.round_corners) * .06}em` } as CSSProperties}>
    <div className="hud-box">{rows}</div>
  </div>;
}

const Check = () => <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"><path d="m5 12 5 5 9-10" /></svg>;

/** New interface: the MangoHUD profile in three plain steps beside a live
 *  preview, with every native option still reachable in "Todas as opções".
 *  Loads and saves the same profile as the classic editor. */
export function ModernMangoHudEditor({ onDirtyChange, onInstall, searchControl }: { onDirtyChange: (dirty: boolean) => void; onInstall: () => void; searchControl?: ReactNode }) {
  const [settings, setSettings] = useState<MangoHudSettings>(mangoHudDefaults), [saved, setSaved] = useState("");
  const [ready, setReady] = useState(false), [saving, setSaving] = useState(false), [error, setError] = useState(""), [saveError, setSaveError] = useState("");
  const [previewBusy, setPreviewBusy] = useState(false), [running, setRunning] = useState(false);
  const [checkingGlobal, setCheckingGlobal] = useState(false), [missingGlobal, setMissingGlobal] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false), [query, setQuery] = useState("");
  const [tick, setTick] = useState(0), [graph, setGraph] = useState(() => Array.from({ length: 48 }, () => 16 + Math.random() * 3));
  const [screenWidth, setScreenWidth] = useState(480);
  const screen = useRef<HTMLDivElement>(null), advanced = useRef<HTMLDetailsElement>(null);
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
    if (reduced) return;
    const timer = setInterval(() => { setTick(value => value + 1); setGraph(points => [...points.slice(1), 16 + Math.random() * 3 + (Math.random() < .06 ? 6 : 0)]); }, 450);
    return () => clearInterval(timer);
  }, []);
  useEffect(() => {
    const element = screen.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => setScreenWidth(entry.contentRect.width));
    observer.observe(element);
    return () => observer.disconnect();
  }, [ready]);
  const update = (patch: MangoHudSettings) => { setSettings(current => ({ ...current, ...patch })); setSaveError(""); };
  const set = (key: string, value: boolean | number | string) => update({ [key]: value });
  const toggleGlobal = async () => {
    if (globalLock.current) return;
    if (settings.launcher_enabled) { set("launcher_enabled", false); setMissingGlobal(false); return; }
    globalLock.current = true; setCheckingGlobal(true); setMissingGlobal(false); setError("");
    try { const status = await getMangoHudStatus(); if (!mounted.current) return; if (status.installed) set("launcher_enabled", true); else setMissingGlobal(true); }
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
  const showAdvanced = (open: boolean) => { setAdvancedOpen(open); if (open) requestAnimationFrame(() => advanced.current?.scrollIntoView({ block: "start", behavior: "smooth" })); };
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
    if (option.kind === "toggle") return <button type="button" className="mango-switch sm" role="switch" aria-checked={Boolean(value)} aria-label={option.label} onClick={() => set(option.key, !value)} />;
    if (option.kind === "select") return <select aria-label={option.label} value={String(value)} onChange={event => set(option.key, event.target.value)}>{(option.values ?? []).map(item => <option key={item} value={item}>{SELECT_LABELS[option.key]?.[item] ?? (item === "" ? "(desligado)" : item)}</option>)}</select>;
    if (option.kind === "color") return <input type="color" aria-label={option.label} value={`#${String(value)}`} onChange={event => set(option.key, event.target.value.slice(1).toUpperCase())} />;
    if (option.kind === "text") return <input type="text" aria-label={option.label} maxLength={64} value={String(value ?? "")} onChange={event => set(option.key, event.target.value.replace(/[\x00-\x1f\x7f]/g, ""))} />;
    return <input type="number" aria-label={option.label} min={option.min} max={option.max} step={option.step} value={Number(value)} onChange={event => { const number = event.target.valueAsNumber; if (Number.isFinite(number)) set(option.key, Math.min(option.max ?? 1000, Math.max(option.min ?? 0, number))); }} />;
  };
  const status = saveError ? "Falha ao salvar" : saving ? "Salvando…" : dirty ? "Aguardando salvamento…" : "Salvo automaticamente";

  if (!ready) return <div className="mango-modern"><div className="mm-head-controls">{searchControl}</div><section className="mm-card"><p>{error || "Carregando configuração…"}</p>{error && <button type="button" className="mm-ghost" onClick={() => void load()}>Tentar novamente</button>}</section></div>;
  return <div className="mango-modern">
    <div className="mm-head">
      <div className="mm-head-copy"><h1>Monitoramento no jogo</h1><p>Mostre FPS, temperatura e uso de hardware por cima do jogo, com o MangoHUD.</p></div>
      <div className="mm-head-controls"><div className="mm-segmented" role="group" aria-label="Modo"><button type="button" aria-pressed={!advancedOpen} onClick={() => showAdvanced(false)}>Simples</button><button type="button" aria-pressed={advancedOpen} onClick={() => showAdvanced(true)}>Avançado</button></div>
      <span className={`mm-saved${saveError ? " failed" : dirty || saving ? " pending" : ""}`} role="status"><i />{status}</span>{searchControl}</div>
    </div>
    {saveError && <div role="alert" className="preferences-error"><p>Não foi possível salvar automaticamente: {saveError}</p><div className="monitoring-actions"><button type="button" disabled={saving} onClick={() => setSaveError("")}>Tentar novamente</button><button type="button" disabled={saving} onClick={() => { setSettings(JSON.parse(saved)); setSaveError(""); }}>Desfazer alterações</button></div></div>}
    {error && <p role="alert" className="preferences-error">{error}</p>}
    <div className="mm-grid">
      <div className="mm-stack">
        <section className="mm-card mm-master">
          <div className="mm-big-icon"><svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2"><path d="M3 12h4l3-8 4 16 3-8h4" /></svg></div>
          <div><strong>Mostrar nos jogos</strong><small>Em todos os jogos abertos pelo PeliGames. No jogo, <kbd>{keyLabel(String(settings.toggle_hud || "Shift_R+F12"))}</kbd> mostra ou esconde.</small>
            {checkingGlobal && <small role="status">Verificando MangoHUD…</small>}
            {missingGlobal && <small role="alert">MangoHUD não foi detectado. <button type="button" className="mm-link" onClick={onInstall}>Clique aqui para instalar</button></small>}
          </div>
          <div className="mm-spacer" />
          <button type="button" className="mango-switch" role="switch" aria-checked={Boolean(settings.launcher_enabled)} aria-label="Mostrar nos jogos" disabled={checkingGlobal} onClick={() => void toggleGlobal()} />
        </section>

        <section className="mm-card mm-system-sync">
          <div><strong>Aplicar também ao MangoHUD do sistema</strong><p>Atualiza a configuração do seu usuário para jogos fora do launcher. A configuração anterior fica preservada em backup.</p></div>
          <button type="button" className="mango-switch" role="switch" aria-label="Aplicar também ao MangoHUD do sistema" aria-checked={Boolean(settings.launcher_sync_system)} onClick={()=>set("launcher_sync_system",!settings.launcher_sync_system)} />
        </section>

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
              const active = on ? settings[key] === on : Boolean(settings[key]);
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
            <div className="mm-row"><span>Tamanho do texto</span>{slider("font_size", 14, 40, 1, value => `${value}px`, "Tamanho do texto")}</div>
            <div className="mm-row"><span>Fundo<small>Quanto o painel esconde o jogo</small></span>{slider("background_alpha", 0, 1, .05, value => `${Math.round(value * 100)}%`, "Fundo")}</div>
            <div className="mm-row"><span>Cantos arredondados</span>{slider("round_corners", 0, 20, 1, value => String(value), "Cantos arredondados")}</div>
            <div className="mm-row"><span>Cores</span><div className="mm-palettes">{PALETTES.map(palette => <button type="button" key={palette.name} className="mm-palette"
              aria-pressed={palette.colors[0] === settings.text_color && palette.colors[1] === settings.gpu_color && palette.colors[2] === settings.cpu_color}
              onClick={() => update({ text_color: palette.colors[0], gpu_color: palette.colors[1], cpu_color: palette.colors[2], ram_color: palette.colors[3], vram_color: palette.colors[4], frametime_color: palette.colors[1] })}>
              <span aria-hidden="true">{palette.colors.slice(1).map((color, index) => <i key={index} style={{ background: `#${color}` }} />)}</span>{palette.name}</button>)}</div></div>
            <div className="mm-row"><span>Contorno no texto<small>Ajuda a ler em cenas claras</small></span><button type="button" className="mango-switch sm" role="switch" aria-label="Contorno no texto" aria-checked={Boolean(settings.text_outline)} onClick={() => set("text_outline", !settings.text_outline)} /></div>
          </div>
        </section>

        <section className="mm-card">
          <h2><span className="mm-step">⚡</span>Limitar FPS</h2>
          <p className="mm-hint">Menos FPS = menos calor, ruído e consumo de bateria. Útil em jogos leves.</p>
          <div className="mm-limit">
            <div className="mm-segmented">{LIMITS.map(([value, label]) => <button type="button" key={value} aria-pressed={!customLimit && limit === value} onClick={() => set("fps_limit", value)}>{label}</button>)}
              <button type="button" aria-pressed={customLimit} onClick={() => set("fps_limit", customLimit ? limit : 90)}>Outro</button></div>
            {customLimit && <input className="mm-limit-input" type="number" min={1} max={1000} aria-label="Limite personalizado" value={limit} onChange={event => { const number = event.target.valueAsNumber; if (Number.isFinite(number)) set("fps_limit", Math.max(1, Math.min(1000, Math.round(number)))); }} />}
          </div>
        </section>

        <details ref={advanced} className="mm-advanced" open={advancedOpen} onToggle={event => setAdvancedOpen(event.currentTarget.open)}>
          <summary><span className="mm-step">⚙</span>Todas as opções <small>· {options.length - 1} opções do MangoHUD, para quem quer tudo</small><svg className="mm-chevron" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4"><path d="m9 6 6 6-6 6" /></svg></summary>
          <div className="mm-advanced-body">
            <label className="mm-search"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg><input value={query} onChange={event => setQuery(event.target.value)} placeholder="Buscar opção (ex.: vsync, bateria, ventoinha)" /></label>
            {sections.map(section => <div key={section} className="mm-advanced-section"><h4>{SECTION_NAMES[section] ?? section}</h4>
              {filtered.filter(option => option.section === section).map(option => <div key={option.key} className="mm-advanced-row"><span>{option.label}</span>{advancedControl(option)}</div>)}
            </div>)}
            {!sections.length && <p className="mm-empty">Nenhuma opção com “{query}”.</p>}
          </div>
        </details>
      </div>

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
  </div>;
}
