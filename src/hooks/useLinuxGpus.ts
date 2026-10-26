import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
export type LinuxGpu = { model:string; pciAddress:string; primary:boolean };
export function useLinuxGpus() {
  const [gpus,setGpus]=useState<LinuxGpu[]>([]);
  const [error,setError]=useState("");
  useEffect(()=>{let active=true;void invoke<LinuxGpu[]>("detect_linux_gpus").then(value=>{if(active)setGpus(value)}).catch(()=>{if(active)setError("Não foi possível identificar as GPUs. A seleção salva foi preservada.")});return()=>{active=false}},[]);
  return {gpus,error};
}
export function gpuChoices(gpus:LinuxGpu[],saved:string) {
  const choices=[{value:"",label:"Seleção automática"},...gpus.map(gpu=>({value:gpu.pciAddress,label:`${gpu.model} · ${gpu.pciAddress}`}))];
  if(saved&&!choices.some(choice=>choice.value===saved))choices.push({value:saved,label:`GPU salva · ${saved} (indisponível)`});
  return choices;
}
