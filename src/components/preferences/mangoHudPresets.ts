import catalog from "../../config/mangohudOptions.json";
import type { MangoHudSettings } from "../../services/mangohud";
const options = catalog;

// Presets put hardware sensors first, followed by FPS and frame timing.
export const PRESETS = [
  { id: "basic", name: "Essencial", description: "FPS, CPU, GPU e memória." },
  { id: "full", name: "Detalhado", description: "Sensores, frequências e gráfico." },
  { id: "horizontal", name: "Faixa", description: "Tudo numa linha no topo." },
  { id: "fps", name: "Só FPS", description: "Discreto, só a taxa de quadros." },
];
export function applyPreset(kind: string, base: MangoHudSettings): MangoHudSettings {
  // Presets explicitly disable Vulkan and OpenGL VSync, including saved profiles.
  const next = { ...base, vsync: "1", gl_vsync: "0" } as MangoHudSettings;
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
