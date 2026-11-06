import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n/I18nContext";
import { MenuIcon } from "./MenuIcon";
interface RuntimeActivity { id:string; runtime:string; phase:"preparing"|"downloading"|"extracting"|"failed"; downloaded_bytes?:number|null; }
export function RuntimeActivityNotice() {
  const {t: tx} = useI18n();

  const [items,setItems]=useState<RuntimeActivity[]>([]);
  const preview=import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window;
  useEffect(()=>{
    let active=true, timer:ReturnType<typeof setTimeout>, last="";
    const poll=async()=>{
      try {
        const next=await invoke<RuntimeActivity[]>("get_runtime_activity");
        const serialized=JSON.stringify(next);
        if(active && serialized!==last) {last=serialized;setItems(next);}
      } catch { /* The launcher error channel still reports execution failures. */ }
      if(active) timer=setTimeout(poll,document.hidden?5000:1500);
    };
    const simulated=(event:Event)=>setItems((event as CustomEvent<RuntimeActivity[]>).detail);
    if(preview) window.addEventListener("previewRuntimeActivity",simulated); else void poll();
    return ()=>{active=false;clearTimeout(timer);window.removeEventListener("previewRuntimeActivity",simulated);};
  },[preview]);
  if(!items.length)return null;
  return <aside className="runtime-activity-notice dialog-popover" aria-label={tx("ui-runtime-activity-notice", "runtime_preparation")}>
    {items.map(item=><section key={item.id}>
      <div className="runtime-activity-heading" role="status"><MenuIcon name={item.phase==="failed"?"info":"download"}/><strong>{(item.phase==="downloading" ? tx("ui-runtime-activity-notice", "downloading_game_runtime") : (item.phase==="extracting" ? tx("ui-runtime-activity-notice", "extracting_runtime") : (item.phase==="failed" ? tx("ui-runtime-activity-notice", "runtime_preparation_failed") : tx("ui-runtime-activity-notice", "preparing_game_environment"))))}</strong></div>
      <small>{preview && (tx("ui-runtime-activity-notice", "preview"))}{item.runtime.replace(/^steamrt(\d+)/, "Steam Runtime $1")}{item.downloaded_bytes!=null && <span aria-live="off"> · {(item.downloaded_bytes/1048576).toFixed(1)} MiB {tx("ui-runtime-activity-notice", "received")}</span>}</small>
      {item.phase!=="failed" && <div className="runtime-activity-progress" aria-hidden="true"><i/></div>}
      <p>{(item.phase==="failed" ? tx("ui-runtime-activity-notice", "runtime_preparation_failed_check_your_connection_and_retry_missing_runtime_files_require") : tx("ui-runtime-activity-notice", "the_game_waits_for_preparation_slow_connections_may_take_longer_the_runtime_is_kept_for_"))}</p>
    </section>)}
  </aside>;
}
