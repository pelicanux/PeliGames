import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {NexusPopup} from "../modules/nexus/NexusPopup";
import {MenuIcon} from "./MenuIcon";
import "./GameCopyDialog.css";
interface RemovedDestination {name:string;directory:string;prefix:string;token:string;reset_reason:string|null}
interface Pending {details:RemovedDestination;executable:string;finish:(value:boolean)=>void}
export function useRemovedPrefixNotice(onRecreated:(path:string)=>void){
 const [pending,setPending]=useState<Pending|null>(null),[acknowledged,setAcknowledged]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState("");
 const active=useRef<Pending|null>(null),checking=useRef(false),mounted=useRef(true);
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;active.current?.finish(false);active.current=null;};},[]);
 const inspect=async(directory:string,executable:string)=>{
  if(checking.current||active.current)return false;
  checking.current=true;
  try{
   if("__PELI_UI_PREVIEW__" in window)return true;
   const details=await invoke<RemovedDestination|null>("check_removed_installation_destination",{directory});
   if(!mounted.current)return false;
   if(!details)return true;
   setError("");setAcknowledged(false);
   return await new Promise<boolean>(finish=>{const next={details,executable,finish};active.current=next;setPending(next);});
  }finally{checking.current=false;}
 };
 const finish=(value:boolean)=>{if(busy)return;active.current?.finish(value);active.current=null;setPending(null);};
 const reset=async()=>{
  if(!pending||!acknowledged||busy)return;
  setBusy(true);setError("");
  try{
   await invoke("reset_removed_installation_destination",{directory:pending.details.directory,token:pending.details.token,executable:pending.executable});
   onRecreated(pending.details.directory);
   active.current?.finish(true);active.current=null;setPending(null);
  }catch(error){setError(String(error));}finally{if(mounted.current)setBusy(false);}
 };
 const selectedInside=pending&&[pending.details.directory,pending.details.prefix].some(path=>pending.executable===path||pending.executable.startsWith(`${path}/`));
 const resetReason=pending?.details.reset_reason||(selectedInside?"O executável selecionado está nesta pasta e seria apagado. Para recriar, escolha primeiro um executável ou instalador fora dela.":null);
 const dialog=pending?<RemovedPrefixPrompt details={pending.details} acknowledged={acknowledged} busy={busy} error={error} resetReason={resetReason} onAcknowledge={setAcknowledged} onCancel={()=>finish(false)} onReuse={()=>finish(true)} onReset={()=>void reset()}/>:null;
 return {inspect,dialog};
}

export function RemovedPrefixPrompt({details,acknowledged,busy,error,resetReason,onAcknowledge,onCancel,onReuse,onReset}:{
 details:RemovedDestination;acknowledged:boolean;busy:boolean;error:string;resetReason:string|null;
 onAcknowledge:(value:boolean)=>void;onCancel:()=>void;onReuse:()=>void;onReset:()=>void;
}){
 return <NexusPopup className="game-copy-dialog removed-prefix-notice" title="Prefixo já existente" titleIcon={<MenuIcon name="folder"/>} onClose={onCancel} footer={<>
   <button type="button" disabled={busy} onClick={onCancel}>Cancelar</button>
   <button type="button" disabled={busy} onClick={onReuse}>Reaproveitar prefixo</button>
   <button type="button" className="removed-prefix-reset" disabled={busy||!acknowledged||Boolean(resetReason)} onClick={onReset}>{busy?"Recriando…":"Apagar e recriar"}</button>
  </>}>
   <div className="game-copy-form">
    <p className="game-copy-note"><strong>{details.name}</strong> foi removido da biblioteca, mas seus arquivos e o prefixo continuam neste local. Continuar reaproveitará o prefixo existente; não criará uma cópia independente.</p>
    <div className="game-copy-destination"><span>Prefixo existente<code>{details.prefix}</code></span></div>
    <div className="game-copy-explanation">
     <p>Para começar do zero, use <strong>Apagar e recriar</strong>. Todos os arquivos da pasta abaixo serão removidos, incluindo jogos, saves e configurações locais. Depois, você continuará esta operação com uma pasta nova no mesmo local.</p>
     <code className="removed-prefix-path">{details.directory}</code>
     <label className="game-copy-acknowledgement"><input type="checkbox" checked={acknowledged} disabled={busy||Boolean(resetReason)} onChange={event=>onAcknowledge(event.target.checked)}/>Entendo que os arquivos desta pasta serão apagados definitivamente.</label>
    </div>
    {resetReason&&<p className="game-copy-note">{resetReason}</p>}
    {error&&<p role="alert" className="game-copy-error">{error}</p>}
   </div>
  </NexusPopup>;
}
