import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import "./capturePreview.css";
type Status={phase:string;recording:boolean;replay:boolean};
export function CaptureIndicator(){
 const [status,setStatus]=useState<Status>(),[busy,setBusy]=useState(false),[error,setError]=useState("");
 useEffect(()=>{let active=true;let unlisten:(()=>void)|undefined;const update=(s:Status)=>{if(active)setStatus(s)};const demo=(e:Event)=>update((e as CustomEvent<Status>).detail);
 window.addEventListener("peligames-capture-preview",demo);
 void(async()=>{try{unlisten=await listen<Status>("peligames-capture",e=>update(e.payload));if(!active){unlisten();return;}update(await invoke<Status>("capture_status"));}catch{/* Capture is optional; the full panel reports detection errors. */}})();
 return()=>{active=false;unlisten?.();window.removeEventListener("peligames-capture-preview",demo);};},[]);
 if(!status||status.phase==="idle")return null;
 const stop=async()=>{setBusy(true);setError("");try{setStatus(await invoke<Status>("capture_action",{action:"stop"}));}catch(e){setError(String(e));}finally{setBusy(false);}};
 return <button type="button" className="capture-titlebar-indicator" data-state={status.phase==="starting"?"starting":status.recording?"recording":"replay"} data-tauri-drag-region="false" disabled={busy} title={error||(status.phase==="starting"?"Cancelar preparação da captura":"Encerrar a captura do PeliGames")} aria-label={status.phase==="starting"?"Cancelar captura":"Encerrar captura"} onClick={()=>void stop()}><span className="capture-indicator-dot" aria-hidden="true"/><span>{status.phase==="starting"?"Preparando":status.recording?"Gravando":"Replay ativo"}</span><span className="capture-indicator-stop" aria-hidden="true"><svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor"><rect x="4" y="4" width="8" height="8" rx="1.5"/></svg></span></button>;
}
