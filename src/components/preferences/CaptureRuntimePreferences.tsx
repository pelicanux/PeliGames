import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {CaptureRuntimeManager} from "../../modules/capture/CaptureRuntimeManager";

export function CaptureRuntimePreferences({disabled,onBusy}:{disabled:boolean;onBusy:(busy:boolean)=>void}) {
 const [active,setActive]=useState(true),[error,setError]=useState("");
 useEffect(()=>{let mounted=true;let off:(()=>void)|undefined;const update=(value:{phase:string})=>{if(mounted)setActive(value.phase!=="idle")};const demo=(e:Event)=>update((e as CustomEvent<{phase:string}>).detail);
 window.addEventListener("peligames-capture-preview",demo);
 void(async()=>{try{off=await listen<{phase:string}>("peligames-capture",e=>update(e.payload));if(!mounted){off();return;}update(await invoke("capture_status"));}catch(e){if(mounted)setError(String(e));}})();
 return()=>{mounted=false;off?.();window.removeEventListener("peligames-capture-preview",demo);};},[]);
 return <section data-preference-id="runtime-capture" className="frame-generation-panel fg-runtime-management mod-workspace"><header className="mod-intro"><h3>Preparar captura de gameplay</h3><p>Grave sessões e salve replays instantâneos. Prepare o componente aqui e configure a captura nos detalhes de cada jogo.</p></header><CaptureRuntimeManager locked={disabled||active} onBusy={onBusy}/>{active&&!error&&<p role="status">Encerre a captura para alterar este componente.</p>}{error&&<p role="alert" className="mod-feedback-error">{error}</p>}</section>;
}
