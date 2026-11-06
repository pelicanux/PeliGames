import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {useI18n} from "../i18n/I18nContext";
import "./ConnectionStorageChoice.css";
interface StorageStatus{temporary:boolean;in_use:boolean}
export function ConnectionStorageChoice({service,disabled=false,onChange}:{service:"nexus"|"discord"|"matrix";disabled?:boolean;onChange?:(temporary:boolean)=>void}){
  const {t: tx} = useI18n();

 const [status,setStatus]=useState<StorageStatus|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState("");
 const preview="__PELI_UI_PREVIEW__" in window;
 useEffect(()=>{
  let active=true,fetching=false;
  const refresh=()=>{if(preview){setStatus({temporary:sessionStorage.getItem(`preview-storage-${service}`)==="temporary",in_use:false});return;}
   if(fetching)return;fetching=true;
   void invoke<StorageStatus>("connection_storage_status",{service}).then(value=>{if(active)setStatus(value);}).catch(()=>{if(active)setError(tx("ui-connection-storage-choice", "could_not_check_connection_storage"));}).finally(()=>{fetching=false;});};
  refresh();const timer=setInterval(refresh,2000);window.addEventListener("connectionStorageChanged",refresh);
  return()=>{active=false;clearInterval(timer);window.removeEventListener("connectionStorageChanged",refresh);};
 },[service,preview,tx]);
 const choose=async(temporary:boolean)=>{
  if(disabled||busy||!status||status.in_use||status.temporary===temporary)return;
  setBusy(true);setError("");
  try{const next=preview?{temporary,in_use:false}:await invoke<StorageStatus>("set_connection_temporary",{service,temporary});
   if(preview)sessionStorage.setItem(`preview-storage-${service}`,temporary?"temporary":"keyring");
   setStatus(next);onChange?.(temporary);window.dispatchEvent(new Event("connectionStorageChanged"));
  }catch(error){setError(typeof error==="string"?error:(tx("ui-connection-storage-choice", "could_not_change_connection_storage")));}finally{setBusy(false);}
 };
 return <div className="connection-storage-choice">
  <div className="connection-storage-options" role="group" aria-label={tx("ui-connection-storage-choice", "connection_storage")}>
   <span>{tx("ui-connection-storage-choice", "session")}</span>
   <button type="button" aria-pressed={status?.temporary===false} disabled={disabled||busy||!status||status.in_use} onClick={()=>void choose(false)}>{tx("ui-connection-storage-choice", "save_in_keyring")}</button>
   <button type="button" aria-pressed={status?.temporary===true} disabled={disabled||busy||!status||status.in_use} onClick={()=>void choose(true)}>{tx("ui-connection-storage-choice", "temporary")}</button>
  </div>
  {status?.temporary?<p role="status">{tx("ui-connection-storage-choice", "for_systems_without_an_active_keyring_keys_and_tokens_stay_only_in_this_process_s_memory")}</p>:<p>{tx("ui-connection-storage-choice", "keeps_you_signed_in_between_launches_requires_an_available_unlocked_compatible_keyring_o")}</p>}
  {status?.in_use&&<small>{tx("ui-connection-storage-choice", "disconnect_before_changing_modes_existing_keyring_credentials_are_not_migrated_into_temp")}</small>}
  {error&&<p role="alert" className="mod-feedback-error">{error}</p>}
 </div>;
}
