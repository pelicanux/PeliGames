import { invoke } from "@tauri-apps/api/core";
import { comparisonPng, type ComparisonReport } from "./mangoHudComparisonReport";

export async function saveBenchmarkComparison(report: ComparisonReport): Promise<boolean> {
  const blob = await comparisonPng(report);
  const game=(report.game || "benchmarks").replace(/[^\p{L}\p{N}_ -]/gu,"_").trim().slice(0,50) || "benchmarks";
  const now=new Date(), stamp=[now.getFullYear(),String(now.getMonth()+1).padStart(2,"0"),String(now.getDate()).padStart(2,"0")].join("-");
  const filename=`comparacao-${game}-${stamp}.png`;
  if ("__PELI_UI_PREVIEW__" in window || !("__TAURI_INTERNALS__" in window)) {
    const url=URL.createObjectURL(blob), anchor=document.createElement("a");
    anchor.href=url;anchor.download=filename;anchor.click();
    // Give WebKit time to start downloading before releasing the object URL.
    setTimeout(()=>URL.revokeObjectURL(url),60000);
    return true;
  }
  const png=await new Promise<string>((resolve,reject)=>{
    const reader=new FileReader();reader.onload=()=>resolve(String(reader.result).split(",")[1]);reader.onerror=()=>reject(new Error("Não foi possível preparar a imagem."));reader.readAsDataURL(blob);
  });
  return invoke<boolean>("save_mangohud_comparison",{png,filename});
}
