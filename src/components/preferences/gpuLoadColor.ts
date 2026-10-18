/** Match MangoHUD's low → medium → high load color interpolation. */
export function gpuLoadColor(load: number, thresholds: string, colors: string): string {
  const [medium, high] = thresholds.split(",").map(Number);
  const palette = colors.split(",");
  const upper = load >= medium;
  const start = palette[upper ? 1 : 0], end = palette[upper ? 2 : 1];
  const ratio = Math.max(0, Math.min(1, upper ? (load - medium) / (high - medium) : load / medium));
  const channels = [0, 2, 4].map(offset => {
    const a = parseInt(start.slice(offset, offset + 2), 16);
    const b = parseInt(end.slice(offset, offset + 2), 16);
    return Math.round(a + (b - a) * ratio);
  });
  return `rgb(${channels.join(", ")})`;
}
