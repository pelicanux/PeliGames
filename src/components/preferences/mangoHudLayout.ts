import blocks from "../../config/mangohudLayout.json";
import type { MangoHudSettings } from "../../services/mangohud";
export const HUD_BLOCKS = blocks;
export function hudOrder(settings: MangoHudSettings): string[] {
  const custom = String(settings.launcher_hud_custom_order || "").split(",");
  if (custom[0]) return [...new Set([...custom.filter(id => blocks.some(b => b.id === id)), ...blocks.map(b => b.id)])];
  if (settings.launcher_hud_order === "hardware-first") {
    return [...blocks].sort((a,b)=>a.hardware_order-b.hardware_order).map(b=>b.id);
  }
  return blocks.map(b=>b.id);
}
/** Move a complete block to the target slot, keeping disabled blocks in the profile. */
export function placeHudBlock(settings: MangoHudSettings, id: string, target: string): string {
  const order=hudOrder(settings);
  const from=order.indexOf(id), to=order.indexOf(target);
  if(from<0 || to<0 || from===to) return order.join(",");
  order.splice(from,1);
  order.splice(to,0,id);
  return order.join(",");
}
