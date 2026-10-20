import { notifyInfo } from "./notifications";
import { invoke } from "@tauri-apps/api/core";
export interface MangoHudStatus { installed: boolean; executable: string | null; }
export interface MangoHudRelease { version: string; release_url: string; download_url: string; asset_name: string; size: number; }
export const getMangoHudStatus = () => mangoHudDemoInstalled() ? Promise.resolve({installed:true,executable:"/preview/usr/bin/mangohud"}) : invoke<MangoHudStatus>("mangohud_status");
export const getMangoHudRelease = () => invoke<MangoHudRelease>("mangohud_latest_release");
export const openMonitoringPreferences = () => window.dispatchEvent(new Event("openMonitoringPreferences"));
export type MangoHudSettings = Record<string, boolean | number | string>;
export interface MangoHudProfile { settings: MangoHudSettings; config_path: string; }
export interface MangoHudPreviewStatus { running: boolean; program: string; }
let profileWrites:Promise<unknown>=Promise.resolve();
export const loadMangoHudProfile = async () => { await profileWrites.catch(()=>{}); return invoke<MangoHudProfile>("load_mangohud_profile"); };
export const saveMangoHudProfile = (settings: MangoHudSettings) => {
  const snapshot={...settings};
  const task=profileWrites.catch(()=>{}).then(()=>invoke<string>("save_mangohud_profile",{settings:snapshot})).then(path => {window.dispatchEvent(new CustomEvent("mangohudProfileChanged",{detail:snapshot}));return path;});
  profileWrites=task;return task;
};
export const updateMangoHudPreview = (settings: MangoHudSettings) => invoke<void>("update_mangohud_preview", { settings });
export const mangoHudPreview = (action: "start" | "stop" | "status", settings?: MangoHudSettings) => invoke<MangoHudPreviewStatus>("mangohud_preview", { action, settings });
export interface MangoHudReady { version:string; sha256:string; size:number; }
export interface MangoHudProgress { phase:string; downloaded:number; total:number; speed_bytes_per_sec:number; }
let demoReady:MangoHudReady|null=null, demoCancel=false, demoInstalled=false;
const demo = () => "__PELI_UI_PREVIEW__" in window;
export const getDownloadedMangoHud = () => demo() ? Promise.resolve(demoReady) : invoke<MangoHudReady|null>("mangohud_downloaded");
export async function downloadMangoHud(version:string,progress:(p:MangoHudProgress)=>void):Promise<MangoHudReady>{
  if(demo()){
    demoCancel=false; const size=10282346;
    for(let i=0;i<=20;i++) { if(demoCancel)throw new Error("Download cancelado.");progress({phase:"downloading",downloaded:Math.round(size*i/20),total:size,speed_bytes_per_sec:size/4});await new Promise(resolve=>setTimeout(resolve,200)); }
    if(demoCancel)throw new Error("Download cancelado.");progress({phase:"verifying",downloaded:size,total:size,speed_bytes_per_sec:0});await new Promise(resolve=>setTimeout(resolve,350));
    demoReady={version,sha256:"a".repeat(64),size};return demoReady;
  }
  const {listen}=await import("@tauri-apps/api/event");
  const stop=await listen<MangoHudProgress>("mangohud-download-progress",e=>progress(e.payload));
  try{return await invoke("download_mangohud",{version});}finally{stop();}
}
export const cancelMangoHudDownload = () => { if(demo()){demoCancel=true;return Promise.resolve();}return invoke<void>("cancel_mangohud_download"); };
export async function installMangoHud(progress:(p:MangoHudProgress)=>void):Promise<void>{
  if(demo()) { if(!demoReady)throw new Error("Baixe o MangoHUD antes de instalar.");progress({phase:"installing",downloaded:0,total:0,speed_bytes_per_sec:0});await new Promise(resolve=>setTimeout(resolve,1800));demoInstalled=true;localStorage.setItem("peligames.preview.mangohudInstalled","true");return; }
  const {listen}=await import("@tauri-apps/api/event");const stop=await listen<MangoHudProgress>("mangohud-download-progress",e=>progress(e.payload));
  try{await invoke("install_mangohud");}finally{stop();}
}
export const mangoHudDemoInstalled = () => demo() && (demoInstalled || localStorage.getItem("peligames.preview.mangohudInstalled") === "true");

let installationRequested = false;
export function consumeMangoHudInstallationRequest() {
  const requested = installationRequested; installationRequested = false; return requested;
}
export function openMangoHudInstallation() {
  installationRequested = true;
  openMonitoringPreferences();
  window.dispatchEvent(new Event("showMangoHudInstallation"));
}
export function notifyMangoHudMissing() {
  notifyInfo("MangoHUD", "MangoHUD não foi detectado no sistema. Clique aqui para instalar em Configurações → Monitoramento.", "mangohud-install");
}

export interface MangoHudPreset {name:string;settings:MangoHudSettings}
const previewPresetKey="peligames.preview.mangohudPresets";
export const listMangoHudPresets=():Promise<MangoHudPreset[]>=>demo()?Promise.resolve(JSON.parse(localStorage.getItem(previewPresetKey)||"[]")):invoke("list_mangohud_presets");
export async function saveMangoHudPreset(name:string,settings:MangoHudSettings):Promise<MangoHudPreset[]> {
 if(!demo())return invoke("save_mangohud_preset",{name,settings});
 const presets=await listMangoHudPresets();name=name.trim();
 if(!name||name.length>64)throw new Error("Use um nome de 1 a 64 caracteres.");
 if(presets.some(p=>p.name.toLowerCase()===name.toLowerCase()))throw new Error("Já existe um preset com esse nome.");
 if(presets.length>=32)throw new Error("Você pode guardar até 32 presets personalizados.");
 const snapshot={...settings};delete snapshot.launcher_enabled;delete snapshot.launcher_sync_system;
 presets.push({name,settings:snapshot});localStorage.setItem(previewPresetKey,JSON.stringify(presets));return presets;
}
export async function removeMangoHudPreset(name:string):Promise<MangoHudPreset[]> {
 if(!demo())return invoke("remove_mangohud_preset",{name});
 const presets=(await listMangoHudPresets()).filter(p=>p.name!==name);localStorage.setItem(previewPresetKey,JSON.stringify(presets));return presets;
}
