import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {useI18n} from "../i18n/I18nContext";
import "./ConnectionStorageChoice.css";
interface StorageStatus{temporary:boolean;in_use:boolean}
export function ConnectionStorageChoice({service,disabled=false,onChange}:{service:"nexus"|"discord"|"matrix";disabled?:boolean;onChange?:(temporary:boolean)=>void}){
 const pt=useI18n().language==="pt";
 const [status,setStatus]=useState<StorageStatus|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState("");
 const preview="__PELI_UI_PREVIEW__" in window;
 useEffect(()=>{
  let active=true,fetching=false;
  const refresh=()=>{if(preview){setStatus({temporary:sessionStorage.getItem(`preview-storage-${service}`)==="temporary",in_use:false});return;}
   if(fetching)return;fetching=true;
   void invoke<StorageStatus>("connection_storage_status",{service}).then(value=>{if(active)setStatus(value);}).catch(()=>{if(active)setError(pt?"Não foi possível consultar o modo da conexão.":"Could not check connection storage.");}).finally(()=>{fetching=false;});};
  refresh();const timer=setInterval(refresh,2000);window.addEventListener("connectionStorageChanged",refresh);
  return()=>{active=false;clearInterval(timer);window.removeEventListener("connectionStorageChanged",refresh);};
 },[service,preview,pt]);
 const choose=async(temporary:boolean)=>{
  if(disabled||busy||!status||status.in_use||status.temporary===temporary)return;
  setBusy(true);setError("");
  try{const next=preview?{temporary,in_use:false}:await invoke<StorageStatus>("set_connection_temporary",{service,temporary});
   if(preview)sessionStorage.setItem(`preview-storage-${service}`,temporary?"temporary":"keyring");
   setStatus(next);onChange?.(temporary);window.dispatchEvent(new Event("connectionStorageChanged"));
  }catch(error){setError(typeof error==="string"?error:(pt?"Não foi possível alterar o modo da conexão.":"Could not change connection storage."));}finally{setBusy(false);}
 };
 return <div className="connection-storage-choice">
  <div className="connection-storage-options" role="group" aria-label={pt?"Guardar conexão":"Connection storage"}>
   <span>{pt?"Sessão":"Session"}</span>
   <button type="button" aria-pressed={status?.temporary===false} disabled={disabled||busy||!status||status.in_use} onClick={()=>void choose(false)}>{pt?"Salvar no chaveiro":"Save in keyring"}</button>
   <button type="button" aria-pressed={status?.temporary===true} disabled={disabled||busy||!status||status.in_use} onClick={()=>void choose(true)}>{pt?"Temporária":"Temporary"}</button>
  </div>
  {status?.temporary?<p role="status">{pt?"Indicada para sistemas sem chaveiro ativo. A chave e os tokens ficam somente na memória desta execução, sem serem salvos no disco. Ao encerrar completamente o PeliGames, será necessário entrar novamente. Fechar só a aba ou a janela do chat não encerra a sessão. Credenciais antigas no chaveiro permanecem lá; este modo não as importa nem apaga.":"For systems without an active keyring. Keys and tokens stay only in this process's memory, without being saved to disk. Sign in again after fully exiting PeliGames. Closing a tab or the chat window does not end the session. Existing keyring credentials stay there; this mode neither imports nor deletes them."}</p>:<p>{pt?"Mantém o login entre aberturas. Requer um chaveiro compatível ativo e desbloqueado, em qualquer interface Linux.":"Keeps you signed in between launches. Requires an available, unlocked compatible keyring on any Linux desktop."}</p>}
  {status?.in_use&&<small>{pt?"Desconecte a conta antes de trocar o modo. Credenciais já salvas no chaveiro não são transferidas para a sessão temporária.":"Disconnect before changing modes. Existing keyring credentials are not migrated into temporary sessions."}</small>}
  {error&&<p role="alert" className="mod-feedback-error">{error}</p>}
 </div>;
}
