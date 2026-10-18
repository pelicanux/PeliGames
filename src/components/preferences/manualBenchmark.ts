import { invoke } from "@tauri-apps/api/core";
import type { BenchmarkStats } from "./mangoHudBenchmarks";
export interface ManualBenchmark { id:string; name:string; modified:number; stats:BenchmarkStats }
interface Selected { name:string; csv:string; modified:number; size:number }
export const webBenchmarkPicker = () => "__PELI_UI_PREVIEW__" in window || !("__TAURI_INTERNALS__" in window);
export function validateBenchmarkFile(name:string,size:number) {
  const lower=name.toLowerCase();
  if(!lower.endsWith(".csv") || lower.endsWith("_summary.csv")) throw new Error("Selecione o CSV completo do MangoHUD, não o arquivo summary.");
  if(size>16*1024*1024) throw new Error("O registro deve ter no máximo 16 MiB.");
}
export async function manualBenchmark(file?:File): Promise<ManualBenchmark|null> {
  let selected:Selected|null;
  if(file) {validateBenchmarkFile(file.name,file.size);selected={name:file.name,csv:await file.text(),size:file.size,modified:Math.floor(file.lastModified/1000)};}
  else selected=await invoke<Selected|null>("select_mangohud_benchmark");
  if(!selected)return null;
  validateBenchmarkFile(selected.name,selected.size);
  const stats=await new Promise<BenchmarkStats>((resolve,reject)=>{
    const worker=new Worker(new URL("./mangoHudBenchmark.worker.ts",import.meta.url),{type:"module"});
    worker.onmessage=(event:MessageEvent<{results?:[BenchmarkStats];error?:string}>)=>{
      worker.terminate();
      if(event.data.results?.[0])resolve(event.data.results[0]);else reject(new Error(event.data.error || "Registro inválido."));
    };
    worker.onerror=()=>{worker.terminate();reject(new Error("Não foi possível analisar esse registro."));};
    worker.postMessage([selected.csv]);
  });
  return {id:`manual:${crypto.randomUUID()}`,name:selected.name,modified:selected.modified,stats};
}
