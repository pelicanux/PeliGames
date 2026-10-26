import catalogData from "../config/advancedOptions.json";
interface AdvancedOption {
 id:string;group:string;title:string;description:string;key:string;value:string;enabled:boolean;kind:string;
 min?:number;max?:number;choices?:{value:string;label:string}[];extra_env?:Record<string,string>;vendor?:string;runner_probe?:boolean;
}
const catalog:AdvancedOption[]=catalogData;

import rules from "../config/advancedRules.json";
export { catalog as advancedOptions };
export interface GamescopeSettings {
  model_version: number; enable_upscaling: boolean; enable_limiter: boolean; force_grab_cursor: boolean; hdr?: boolean; adaptive_sync?: boolean;
  width: number; height: number; output_width: number; output_height: number; fps: number; fps_unfocused: number;
  filter: "fsr" | "nis" | "integer" | "stretch"; window_type: "fullscreen" | "borderless" | "windowed";
  additional_options: string;
}
export function gamescopeEnabled(g: GamescopeSettings): boolean {
  return g.enable_upscaling || g.enable_limiter || g.force_grab_cursor || Boolean(g.hdr) || Boolean(g.adaptive_sync) || Boolean(g.additional_options.trim());
}
export interface LaunchWrapper { program: string; arguments: string; enabled: boolean; }
/** Recognize both existing SHA-256 paths and readable per-game profile names. */
export const isFrameGenerationWrapper = (w: LaunchWrapper): boolean => /\/FrameGeneration\/profiles\/(?:[a-f0-9]{64}|[a-z0-9]+(?:-[a-z0-9]+)*-[a-f0-9]{8,64})\.sh$/.test(w.program);
export interface AdvancedSettings {
  mangohud_preset?: import("./mangohud").MangoHudPreset | null;
  overlay_enabled?: boolean | null; background_fps?: number | null; mangohud_enabled?: boolean | null; mangohud_fps_limit?: number | null; schema_version?: number; launch_options?: string | null; options: Record<string, boolean>; option_values?: Record<string,string>; dll_overrides: string;
  wrappers?: LaunchWrapper[]; launch_backend: "system" | "gamescope"; gamescope?: GamescopeSettings;
  afmf?: AfmfSettings | null;
}
/** afmf-linux options for one game; read once when the game starts. */
export interface AfmfSettings {
  enabled: boolean; performance_mode: "auto" | "quality" | "performance"; search_mode: "auto" | "standard" | "high";
  fast_motion: "blend" | "repeat"; min_fps: number; hud_detect: boolean; pacing: boolean;
}
export const defaultAfmf: AfmfSettings = { enabled: false, performance_mode: "auto", search_mode: "auto", fast_motion: "blend", min_fps: 30, hud_detect: true, pacing: true };
export const defaultGamescope: GamescopeSettings = { model_version: 2, enable_upscaling: false, enable_limiter: false, force_grab_cursor: false, hdr: false, adaptive_sync: false, width: 0, height: 0, output_width: 0, output_height: 0, fps: 0, fps_unfocused: 0, filter: "fsr", window_type: "fullscreen", additional_options: "" };
export function normalizeAdvanced(value?: AdvancedSettings): AdvancedSettings {
  // Version 1 accidentally enabled screenshot selections by default. Reset once.
  const saved = value?.schema_version === 2 ? value : undefined;
  return { mangohud_preset: saved?.mangohud_preset ?? null, overlay_enabled: saved?.overlay_enabled ?? null, background_fps: saved?.background_fps ?? null, mangohud_enabled: saved?.mangohud_enabled ?? null, mangohud_fps_limit: saved?.mangohud_fps_limit ?? null, schema_version: 2, option_values: saved?.option_values ?? {}, launch_options: saved?.launch_options ?? null, wrappers: saved?.wrappers ?? [], options: Object.fromEntries(catalog.map(option => [option.id, saved?.options[option.id] ?? false])), dll_overrides: saved?.options.dll_overrides ? saved.dll_overrides ?? "" : "", launch_backend: saved?.launch_backend ?? "system", gamescope: { ...defaultGamescope, ...(saved?.gamescope?.model_version === 2 ? saved.gamescope : {}) }, afmf: saved?.afmf ? { ...defaultAfmf, ...saved.afmf } : null };
}
export function advancedOptionValue(settings:AdvancedSettings,id:string):string {
 return settings.option_values?.[id] ?? catalog.find(option=>option.id===id)?.value ?? "";
}
export function advancedValuesError(settings:AdvancedSettings):string|undefined {
 for(const [id,value] of Object.entries(settings.option_values??{})) {
  const option=catalog.find(option=>option.id===id);
  if(!option || (!option.choices && option.min==null))return "Campo avançado desconhecido.";
  if(option.choices && !option.choices.some(choice=>choice.value===value))return `Valor inválido para ${option.title}.`;
  if(option.min!=null && (!/^\d+$/.test(value)||Number(value)<option.min||Number(value)>(option.max??option.min)))return `Informe ${option.title} entre ${option.min} e ${option.max}.`;
 }
 if(settings.options.dxvk_limit && ((settings.mangohud_fps_limit??0)>0||settings.gamescope?.enable_limiter))return "Escolha apenas um limitador de FPS: DXVK, MangoHUD ou Gamescope.";
 if(settings.options.no_hdr&&settings.gamescope?.hdr)return "HDR do Gamescope conflita com a opção de desligar HDR.";
}
export function toggleAdvanced(settings: AdvancedSettings, id: string): AdvancedSettings {
  return { ...settings, options: { ...settings.options, [id]: !settings.options[id] } };
}
function optionTitle(id: string): string { return catalog.find(option => option.id === id)?.title ?? id; }
function relatedOptions(id: string, dependents = false): string[] {
  const visited = new Set([id]), result: string[] = [];
  const visit = (current: string) => {
    for (const [child, parent] of rules.dependencies) {
      const next = dependents ? child : parent;
      if ((dependents ? parent : child) !== current || visited.has(next)) continue;
      visited.add(next); visit(next); result.push(next);
    }
  };
  visit(id); return result;
}
/** Explain the attempted action, including the order of transitive prerequisites. */
export function advancedToggleError(settings: AdvancedSettings, id: string): string | undefined {
  const turningOff = Boolean(settings.options[id]);
  if(!turningOff&&id==="dxvk_limit"&&((settings.mangohud_fps_limit??0)>0||settings.gamescope?.enable_limiter))return "Desative o limitador MangoHUD/Gamescope antes de ativar o limitador DXVK.";
  if (!turningOff && ["mlfg", "obs_capture", "vkd3d_latency", "vulkan_latency"].includes(id) && settings.wrappers?.some(w => w.enabled && isFrameGenerationWrapper(w))) return "Desative a geração de quadros na aba Geração de quadros antes de ativar esta opção.";

  const required = relatedOptions(id, turningOff).filter(other => turningOff ? settings.options[other] : !settings.options[other]);
  const names = (ids: string[]) => ids.map(other => `“${optionTitle(other)}”`).join(" → ");
  if (!turningOff) {
    const conflicts = rules.conflicts.flatMap(([a, b]) => a === id ? [b] : b === id ? [a] : []).filter(other => settings.options[other]);
    if (conflicts.length) return `Para ativar “${optionTitle(id)}”, desative ${names(conflicts)}. Essas opções são incompatíveis.`;
    if (["mangohud", "mangohud_env"].includes(id) && gamescopeEnabled(settings.gamescope ?? defaultGamescope)) return `Desative os recursos do Gamescope antes de ativar “${optionTitle(id)}”.`;
  }
  if (required.length) return `${turningOff ? "Desative" : "Ative"} primeiro ${names(required)} para ${turningOff ? "desativar" : "ativar"} “${optionTitle(id)}”.`;
}
export function advancedOptionHint(settings: AdvancedSettings, id: string): string | undefined {
  const issue = advancedToggleError(settings, id);
  if (issue) return issue;
  const required = relatedOptions(id);
  if (required.length) return `Requer: ${required.map(optionTitle).join(" → ")}.`;
}
export function advancedError(settings: AdvancedSettings): string | undefined {
  const valuesError=advancedValuesError(settings);if(valuesError)return valuesError;
  if(settings.background_fps!=null && (!Number.isInteger(settings.background_fps)||settings.background_fps<0||settings.background_fps>1000))return "Informe FPS sem foco entre 1 e 1000, ou use Automático.";
  if(settings.background_fps!=null && !settings.overlay_enabled)return "Ative o Overlay PeliGames para limitar FPS sem foco.";
  if (settings.wrappers?.some(w=>w.enabled && isFrameGenerationWrapper(w)) && ["mlfg","obs_capture","vkd3d_latency","vulkan_latency"].some(id=>settings.options[id])) return "Desative a geração de quadros antes de ativar geração ML, baixa latência DX12/Vulkan ou captura Vulkan OBS.";

  const limit = settings.mangohud_fps_limit;
  if (limit != null && (!Number.isInteger(limit) || limit < 0 || limit > 1000)) return "Informe um limite de FPS entre 0 e 1000.";
  for (const [id, dependency, message] of rules.dependencies) if (settings.options[id] && !settings.options[dependency]) return message;
  for (const [a, b, message] of rules.conflicts) if (settings.options[a] && settings.options[b]) return message;
  if (settings.dll_overrides.length > 2048) return "Use até 2048 caracteres nas prioridades de DLLs.";
  if (settings.options.dll_overrides && !/^[\w.*-]+(?:,[\w.*-]+)*=(?:n|b|d|n,b|b,n)?(?:;[\w.*-]+(?:,[\w.*-]+)*=(?:n|b|d|n,b|b,n)?)*$/.test(settings.dll_overrides)) return "Use DLLs no formato dxgi=n,b;dinput8=n,b.";
  if ((settings.wrappers?.length ?? 0) > 16) return "Use até 16 wrappers.";
  for (const wrapper of settings.wrappers ?? []) {
    if (!wrapper.program.trim() || wrapper.program.length > 4096 || /[\x00-\x1f\x7f]/.test(wrapper.program)) return "Informe o executável do wrapper.";
    const error = wrapperArgumentsError(wrapper.arguments); if (error) return error;
  }
  const g = settings.gamescope ?? defaultGamescope;
  if (gamescopeEnabled(g) && (settings.options.mangohud || settings.options.mangohud_env || (limit ?? 0) > 0 || settings.background_fps!=null)) return "Desative o MangoHUD e o limitador individual antes de usar o Gamescope.";
  if (!["fsr", "nis", "integer", "stretch"].includes(g.filter) || !["fullscreen", "borderless", "windowed"].includes(g.window_type)) return "Método de escala ou tipo de janela inválido.";
  const extraError = validateGamescopeExtra(g.additional_options); if (extraError) return extraError;
  for (const key of ["width", "height", "output_width", "output_height", "fps", "fps_unfocused"] as const) {
    if (!Number.isInteger(g[key]) || g[key] < 0 || g[key] > (key.startsWith("fps") ? 1000 : 16384)) return "Informe dimensões válidas (0 a 16384) e FPS entre 0 e 1000. Zero usa o padrão.";
  }
}

export function validateGamescopeExtra(value: string): string | undefined {
  if (value.length > 4096 || /[\x00-\x1f$`;&|<>]/.test(value)) return "Informe apenas argumentos do Gamescope, sem comandos de shell.";
  let quote = "", escape = false, token = ""; const tokens: string[] = [];
  for (const c of value) {
    if (escape) { token += c; escape = false; }
    else if (c === "\\" && quote !== "'") escape = true;
    else if (quote) { if (c === quote) quote = ""; else token += c; }
    else if (c === "'" || c === '"') quote = c;
    else if (/\s/.test(c)) { if (token) { tokens.push(token); token = ""; } }
    else token += c;
  }
  if (quote || escape) return "Feche as aspas das opções adicionais.";
  if (token) tokens.push(token);
  if (tokens.length && !tokens[0].startsWith("-")) return "As opções adicionais precisam começar por uma opção do Gamescope.";
  const reserved = ["-w", "-h", "-W", "-H", "-r", "-o", "-F", "-S", "-f", "-b", "--force-grab-cursor", "--nested-width", "--nested-height", "--output-width", "--output-height", "--nested-refresh", "--nested-unfocused-refresh", "--filter", "--scaler", "--fullscreen", "--borderless", "--expose-wayland", "--hdr-enabled", "--adaptive-sync"];
  if (tokens.some(t => t === "--" || reserved.includes(t.split("=")[0]))) return "Configure resolução, escala, FPS, janela e cursor nos campos próprios; não use -- nas opções adicionais.";
}

export function wrapperArgumentsError(value: string): string | undefined {
  if (value.length > 4096 || /[\x00-\x1f\x7f]/.test(value)) return "Argumentos do wrapper inválidos.";
  let quote = "", escape = false;
  for (const c of value) {
    if (escape) escape = false;
    else if (c === "\\" && quote !== "'") escape = true;
    else if (quote) { if (c === quote) quote = ""; }
    else if (c === "'" || c === '"') quote = c;
  }
  if (quote || escape) return "Feche as aspas dos argumentos do wrapper.";
}

export function buildSteamLaunchOptions(settings: AdvancedSettings): string {
  const envs: string[] = [];
  const wrappers: string[] = [];

  for (const option of catalog) {
    if (option.id === "steam_client") continue;
    if (!settings.options[option.id]) continue;
    if (option.kind === "wrapper") {
      wrappers.push(option.key);
    } else if (option.kind === "env") {
      if (option.id === "dll_overrides") {
        const dlls = settings.dll_overrides.trim() || option.value;
        if (dlls) envs.push(`WINEDLLOVERRIDES="${dlls}"`);
      } else if (option.id === "hdr") {
        envs.push("PROTON_ENABLE_HDR=1");
        envs.push("DXVK_HDR=1");
      } else {
        envs.push(`${option.key}=${advancedOptionValue(settings,option.id)}`);
        for(const [key,value] of Object.entries(option.extra_env??{}))envs.push(`${key}=${value}`);
      }
    }
  }

  if (settings.mangohud_fps_limit && settings.mangohud_fps_limit > 0) {
    envs.push(`MANGOHUD_CONFIG="fps_limit=${settings.mangohud_fps_limit}"`);
  }

  if (settings.wrappers?.length) {
    for (const w of settings.wrappers) {
      if (w.enabled && w.program.trim()) {
        wrappers.push(w.arguments.trim() ? `${w.program.trim()} ${w.arguments.trim()}` : w.program.trim());
      }
    }
  }

  if (settings.gamescope && gamescopeEnabled(settings.gamescope)) {
    const g = settings.gamescope;
    const gArgs: string[] = ["gamescope"];
    if(g.enable_upscaling){
      for(const [flag,value] of [["-w",g.width],["-h",g.height],["-W",g.output_width],["-H",g.output_height]] as const)if(value>0)gArgs.push(`${flag} ${value}`);
      gArgs.push(`${g.filter==="fsr"||g.filter==="nis"?"-F":"-S"} ${g.filter}`);
      if(g.window_type==="fullscreen")gArgs.push("-f");
      else if(g.window_type==="borderless")gArgs.push("-b");
    }
    if(g.enable_limiter&&g.fps>0)gArgs.push(`-r ${g.fps}`);
    if(g.enable_limiter&&g.fps_unfocused>0)gArgs.push(`-o ${g.fps_unfocused}`);
    if(g.force_grab_cursor)gArgs.push("--force-grab-cursor");
    if(settings.options.wayland)gArgs.push("--expose-wayland");
    if(g.hdr)gArgs.push("--hdr-enabled");
    if(g.adaptive_sync)gArgs.push("--adaptive-sync");
    if (g.additional_options?.trim()) gArgs.push(g.additional_options.trim());
    gArgs.push("--");
    wrappers.unshift(gArgs.join(" "));
  }

  const prefix = [...envs, ...wrappers].join(" ").trim();
  if (!prefix) return "";
  return `${prefix} %command%`;
}
