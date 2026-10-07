import catalog from "../config/advancedOptions.json";
import rules from "../config/advancedRules.json";
export { catalog as advancedOptions };
export interface GamescopeSettings {
  model_version: number; enable_upscaling: boolean; enable_limiter: boolean; force_grab_cursor: boolean;
  width: number; height: number; output_width: number; output_height: number; fps: number; fps_unfocused: number;
  filter: "fsr" | "nis" | "integer" | "stretch"; window_type: "fullscreen" | "borderless" | "windowed";
  additional_options: string;
}
export function gamescopeEnabled(g: GamescopeSettings): boolean {
  return g.enable_upscaling || g.enable_limiter || g.force_grab_cursor || Boolean(g.additional_options.trim());
}
export interface LaunchWrapper { program: string; arguments: string; enabled: boolean; }
export interface AdvancedSettings {
  schema_version?: number; options: Record<string, boolean>; dll_overrides: string;
  wrappers?: LaunchWrapper[]; launch_backend: "system" | "gamescope"; gamescope?: GamescopeSettings;
}
export const defaultGamescope: GamescopeSettings = { model_version: 2, enable_upscaling: false, enable_limiter: false, force_grab_cursor: false, width: 0, height: 0, output_width: 0, output_height: 0, fps: 0, fps_unfocused: 0, filter: "fsr", window_type: "fullscreen", additional_options: "" };
export function normalizeAdvanced(value?: AdvancedSettings): AdvancedSettings {
  // Version 1 accidentally enabled screenshot selections by default. Reset once.
  const saved = value?.schema_version === 2 ? value : undefined;
  return { schema_version: 2, wrappers: saved?.wrappers ?? [], options: Object.fromEntries(catalog.map(option => [option.id, saved?.options[option.id] ?? false])), dll_overrides: saved?.dll_overrides ?? "dxgi=n,b", launch_backend: saved?.launch_backend ?? "system", gamescope: { ...defaultGamescope, ...(saved?.gamescope?.model_version === 2 ? saved.gamescope : {}) } };
}
export function toggleAdvanced(settings: AdvancedSettings, id: string): AdvancedSettings {
  return { ...settings, options: { ...settings.options, [id]: !settings.options[id] } };
}
export function advancedError(settings: AdvancedSettings): string | undefined {
  for (const [id, dependency, message] of rules.dependencies) if (settings.options[id] && !settings.options[dependency]) return message;
  for (const [a, b, message] of rules.conflicts) if (settings.options[a] && settings.options[b]) return message;
  if (settings.options.dll_overrides && !/^[\w.*-]+(?:,[\w.*-]+)*=(?:n|b|d|n,b|b,n)?(?:;[\w.*-]+(?:,[\w.*-]+)*=(?:n|b|d|n,b|b,n)?)*$/.test(settings.dll_overrides)) return "Use DLLs no formato dxgi=n,b;dinput8=n,b.";
  if ((settings.wrappers?.length ?? 0) > 16) return "Use até 16 wrappers.";
  for (const wrapper of settings.wrappers ?? []) {
    if (!wrapper.program.trim() || wrapper.program.length > 4096 || /[\x00-\x1f\x7f]/.test(wrapper.program)) return "Informe o executável do wrapper.";
    const error = wrapperArgumentsError(wrapper.arguments); if (error) return error;
  }
  const g = settings.gamescope ?? defaultGamescope;
  if (gamescopeEnabled(g) && (settings.options.mangohud || settings.options.mangohud_env)) return "Desative os modos de MangoHud antes de usar o Gamescope.";
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
  const reserved = ["-w", "-h", "-W", "-H", "-r", "-o", "-F", "-S", "-f", "-b", "--force-grab-cursor", "--nested-width", "--nested-height", "--output-width", "--output-height", "--nested-refresh", "--nested-unfocused-refresh", "--filter", "--scaler", "--fullscreen", "--borderless", "--expose-wayland"];
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
