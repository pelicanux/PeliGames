import { useEffect, useState } from "react";

export const MAX_INTERFACE_SCALE = 1.5;

export function useInterfaceScale() {
  const [scale, setScale] = useState(() => {
    const saved = Number(localStorage.getItem("ui_scale") ?? 1);
    return Number.isFinite(saved) ? Math.min(MAX_INTERFACE_SCALE, Math.max(0.6, saved)) : 1;
  });
  useEffect(() => {
    localStorage.setItem("ui_scale", String(scale));
    document.documentElement.style.fontSize = `${16 * scale}px`;
  }, [scale]);
  return [scale, setScale] as const;
}
