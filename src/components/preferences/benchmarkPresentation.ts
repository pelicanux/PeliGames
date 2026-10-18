import type { BenchmarkStats } from "./mangoHudBenchmarks";
import type { comparisonRows } from "./mangoHudComparisonReport";
export type ComparisonRow = typeof comparisonRows[number];
export const testLabel = (value:string|undefined,index:number) => value?.replace(/\s+/g," ").trim().slice(0,48) || `Teste ${index===0?"A":"B"}`;
export const metricDescriptions: Partial<Record<keyof BenchmarkStats,string>> = {
  averageFps:"Ritmo médio de exibição dos quadros. Maior é melhor.",
  low1:"Estimativa dos 1% piores tempos amostrados. Maior é melhor.",
  low01:"Estimativa dos 0,1% piores tempos amostrados. Maior é melhor.",
  frameTime:"Tempo médio entre quadros. Menor é melhor.",
  cpuLoad:"Ocupação média da CPU; menos uso não significa necessariamente mais desempenho.",
  gpuLoad:"Ocupação média da GPU; mais uso não é, por si só, melhor ou pior.",
  ramUsed:"Memória RAM em uso no sistema, não apenas pelo jogo.",
  vramUsed:"Memória de vídeo em uso informada pelo MangoHUD.",
  gpuPower:"Potência média da GPU durante a gravação.", gpuTemp:"Temperatura média da GPU durante a gravação.",
  duration:"Compare o mesmo trecho e, de preferência, gravações de duração semelhante.", samples:"Quantidade de leituras válidas, não quantidade de quadros renderizados.",
};
export function comparisonInsight(a:BenchmarkStats|null,b:BenchmarkStats|null,row:ComparisonRow) {
  const av=a?.[row.key],bv=b?.[row.key];
  if(typeof av!=="number" || typeof bv!=="number" || !Number.isFinite(av) || !Number.isFinite(bv))return {text:"Sem dados para comparar",tone:"neutral" as const};
  const difference=bv-av;
  if(Math.abs(difference)<1e-9)return {text:"Sem diferença",tone:"neutral" as const};
  const direction=difference>0?"mais":"menos";
  const absolute=Math.abs(difference)<.1?"<0,1":Math.abs(difference).toLocaleString("pt-BR",{maximumFractionDigits:1});
  const delta=av>0 ? Math.abs(difference/av*100) : undefined;
  const percent=delta===undefined?undefined:delta<.1?"<0,1":delta.toLocaleString("pt-BR",{maximumFractionDigits:1});
  let text:string;
  if(["averageFps","low1","low01"].includes(row.key))text=percent ? `${percent}% ${direction} FPS em B` : `${absolute} FPS a ${direction} em B`;
  else if(row.key==="frameTime")text=percent ? `${percent}% ${direction} tempo por quadro em B` : `${absolute} ms a ${direction} em B`;
  else if(row.unit==="%")text=`${absolute} p.p. a ${direction} em B`;
  else text=`${absolute} ${row.unit || "amostras"} a ${direction} em B`;
  const scored=["averageFps","low1","low01","frameTime"].includes(row.key);
  const improved=row.lower?difference<0:difference>0;
  return {text,tone:scored ? improved?"positive" as const:"negative" as const:"neutral" as const};
}
export function metricBarWidths(a:unknown,b:unknown):[number,number] {
  const av=typeof a==="number" && Number.isFinite(a) && a>=0?a:0,bv=typeof b==="number" && Number.isFinite(b) && b>=0?b:0;
  const max=Math.max(av,bv);
  return max>0?[av/max*100,bv/max*100]:[0,0];
}
export const metricGroups = [
  {title:"Fluidez",keys:["averageFps","low1","low01","frameTime"]},
  {title:"Uso de hardware",keys:["cpuLoad","gpuLoad","ramUsed","vramUsed","gpuPower","gpuTemp"]},
  {title:"Sobre as gravações",keys:["duration","samples"]},
] as const;
