import catalog from "../../config/mangohudOptions.json";

const aliases: Record<string, string> = { ctrl_l: "Control_L", ctrl_r: "Control_R", control_l: "Control_L", control_r: "Control_R", shift_l: "Shift_L", shift_r: "Shift_R", alt_l: "Alt_L", alt_r: "Alt_R", super_l: "Super_L", super_r: "Super_R" };
const parts = (value: string) => value.trim().split("+").map(part => {
  const key = part.trim(); return aliases[key.toLowerCase()] ?? key.toLowerCase();
}).filter(Boolean);
/** Modifier order is irrelevant; left/right remain distinct in MangoHUD. */
export function normalizedShortcut(value: string) {
  return [...new Set(parts(value))].sort().join("+");
}
const actionNames: Record<string, string> = {
  toggle_hud: "Mostrar ou ocultar painel", toggle_logging: "Iniciar ou encerrar gravação",
  reset_fps_metrics: "Redefinir médias", toggle_fps_limit: "Alternar limites de FPS",
};
const shortcuts = catalog.filter(option => option.kind === "shortcut");
export function shortcutConflict(key: string, value: string, settings: Record<string, unknown>): string | null {
  const normalized = normalizedShortcut(value);
  if (!normalized) return null;
  const tokens = parts(value);
  // Library search accepts either modifier side and extra modifiers as well.
  if (tokens.includes("k") && tokens.some(token => ["Control_L", "Control_R", "Super_L", "Super_R"].includes(token))) return "Buscar na biblioteca (Ctrl + K / Super + K)";
  const used = shortcuts.find(option => option.key !== key && normalizedShortcut(String(settings[option.key] ?? option.default)) === normalized);
  return used ? actionNames[used.key] ?? used.label : null;
}
