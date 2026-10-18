export interface BenchmarkSample { fps: number; frametime: number; elapsed?: number }
export interface BenchmarkStats { samples: number; averageFps: number; low1: number; low01: number; frameTime: number; gpuPower?: number; gpuTemp?: number; cpuLoad?: number; gpuLoad?: number; ramUsed?: number; vramUsed?: number; duration?: number; graph: number[] }
function cells(line: string): string[] {
  const result: string[] = []; let value = "", quoted = false;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (c === '"') { if (quoted && line[i+1] === '"') { value += '"'; i++; } else quoted = !quoted; }
    else if (c === ',' && !quoted) { result.push(value.trim()); value = ""; }
    else value += c;
  }
  result.push(value.trim()); return result;
}
export function parseBenchmark(csv: string): BenchmarkStats {
  const lines = csv.replace(/^\uFEFF/, "").split(/\r?\n/);
  const start = lines.findIndex(line => { const keys = cells(line).map(key => key.toLowerCase()); return keys.includes("fps") && keys.includes("frametime"); });
  if (start < 0) throw new Error("CSV sem as colunas FPS e frametime do MangoHUD. Escolha o registro completo, não o arquivo summary.");
  const keys = cells(lines[start]).map(key => key.toLowerCase());
  const samples: BenchmarkSample[] = [], power: number[] = [], temperatures: number[] = [], cpuLoads: number[] = [], gpuLoads: number[] = [], ram: number[] = [], vram: number[] = [];
  for (let i = start + 1; i < lines.length; i++) {
    const row = cells(lines[i]);
    const number = (name: string) => { const index = keys.indexOf(name); return index < 0 || !row[index]?.trim() ? NaN : Number(row[index]); };
    const fps = number("fps"), frametime = number("frametime");
    if (!Number.isFinite(fps) || !Number.isFinite(frametime) || fps <= 0 || frametime <= 0) continue;
    samples.push({ fps, frametime, elapsed: number("elapsed") });
    const watts = number("gpu_power"), temp = number("gpu_temp"), cpu = number("cpu_load");
    const gpu = number("gpu_load"), ramValue = number("ram_used"), vramValue = number("gpu_vram_used");
    if (Number.isFinite(gpu) && gpu >= 0 && gpu <= 100) gpuLoads.push(gpu);
    if (Number.isFinite(ramValue) && ramValue >= 0) ram.push(ramValue);
    if (Number.isFinite(vramValue) && vramValue >= 0) vram.push(vramValue);
    if (Number.isFinite(cpu) && cpu >= 0 && cpu <= 100) cpuLoads.push(cpu);
    if (Number.isFinite(watts) && watts > 0) power.push(watts);
    if (Number.isFinite(temp) && temp > 0) temperatures.push(temp);
  }
  if (!samples.length) throw new Error("Esse registro ainda não contém amostras válidas. Encerre a gravação e atualize a lista.");
  const average = (values: number[]) => values.reduce((a,b) => a+b, 0) / values.length;
  const timings = samples.map(s => s.frametime), frameTime = average(timings);
  const sorted = [...timings].sort((a,b) => b-a);
  const low = (fraction: number) => 1000 / average(sorted.slice(0, Math.max(1, Math.ceil(sorted.length * fraction))));
  // Keep average bucket values for a small plot without sending/rendering all samples.
  const buckets = Math.min(120, samples.length), graph = Array.from({ length: buckets }, (_,i) => average(timings.slice(Math.floor(i*samples.length/buckets), Math.floor((i+1)*samples.length/buckets))));
  const last = samples[samples.length-1].elapsed;
  return { samples: samples.length, averageFps: 1000/frameTime, low1: low(.01), low01: low(.001), frameTime,
    gpuLoad: gpuLoads.length ? average(gpuLoads) : undefined,
    ramUsed: ram.length ? average(ram) : undefined, vramUsed: vram.length ? average(vram) : undefined,
    cpuLoad: cpuLoads.length ? average(cpuLoads) : undefined,
    gpuPower: power.length ? average(power) : undefined, gpuTemp: temperatures.length ? average(temperatures) : undefined,
    duration: last !== undefined && Number.isFinite(last) && last > 0 ? last/1e9 : undefined, graph };
}
