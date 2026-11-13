import {useEffect,useRef,useState} from "react";
import {useI18n} from "../../i18n/I18nContext";
import {MenuIcon} from "../../components/MenuIcon";
import {openFilePicker} from "../../services/tauriService";
import type {LocalAddonPlan} from "./contracts";
import type {AddonStoreController} from "./store";

export function AddonLocalImport({controller,disabled}:{controller:AddonStoreController;disabled:boolean}){
 const {t}=useI18n();
 const [selected,setSelected]=useState<{path:string;plan:LocalAddonPlan}>(),[reading,setReading]=useState(false),[error,setError]=useState("");
 const alive=useRef(true),locked=useRef(false);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false}},[]);
 const choose=async()=>{
  if(locked.current||disabled)return;locked.current=true;setReading(true);setError("");
  try{const path=await openFilePicker(t("addons","local_file"),["zip"]);if(!path||!alive.current)return;
   const plan=await controller.inspectLocal(path);if(alive.current)setSelected({path,plan});
  }catch(error){if(alive.current)setError(String(error))}finally{locked.current=false;if(alive.current)setReading(false)}
 };
 const install=async()=>{if(!selected||locked.current||disabled)return;locked.current=true;
  try{const accepted=await controller.importLocal(selected.path,selected.plan.sha256);if(accepted&&alive.current)setSelected(undefined)}finally{locked.current=false}
 };
 return <section className="addon-store-local" aria-label={t("addons","import_local")}>
  <div className="addon-store-local-heading"><button type="button" className="mod-action-secondary" disabled={disabled||reading} onClick={()=>void choose()}><MenuIcon name="folder"/>{t("addons",reading?"local_reading":"import_local")}</button><span>{t("addons","local_hint")}</span></div>
  {error&&<p className="preferences-error" role="alert">{error}</p>}
  {selected&&<div className="addon-store-local-review">
   <div><strong>{selected.plan.manifest.name}</strong><span>{selected.plan.manifest.version} · {t("addons","local_package")}</span></div>
   <p>{selected.plan.manifest.summary}</p><small>{t("addons","local_author",{author:selected.plan.manifest.author})}</small>
   <p>{t("addons","local_review")}</p>
   <div className="addon-store-confirm-actions"><button type="button" className="mod-action-quiet" disabled={disabled} onClick={()=>{setSelected(undefined);setError("")}}>{t("addons","cancel")}</button><button type="button" className="mod-action-primary" disabled={disabled} onClick={()=>void install()}>{t("addons","local_install")}</button></div>
  </div>}
 </section>;
}
