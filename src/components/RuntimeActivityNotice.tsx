import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n/I18nContext";
import { MenuIcon } from "./MenuIcon";
interface RuntimeActivity { id:string; runtime:string; phase:"preparing"|"downloading"|"extracting"|"failed"; downloaded_bytes?:number|null; }
export function RuntimeActivityNotice() {
  const { language }=useI18n(), pt=language==="pt";
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
  return <aside className="runtime-activity-notice dialog-popover" aria-label={pt?"Preparação do runtime":"Runtime preparation"}>
    {items.map(item=><section key={item.id}>
      <div className="runtime-activity-heading" role="status"><MenuIcon name={item.phase==="failed"?"info":"download"}/><strong>{pt ? item.phase==="downloading"?"Baixando runtime do jogo":item.phase==="extracting"?"Extraindo runtime":item.phase==="failed"?"Falha ao preparar o runtime":"Preparando ambiente do jogo" : item.phase==="downloading"?"Downloading game runtime":item.phase==="extracting"?"Extracting runtime":item.phase==="failed"?"Runtime preparation failed":"Preparing game environment"}</strong></div>
      <small>{preview && (pt?"Demonstração · ":"Preview · ")}{item.runtime.replace(/^steamrt(\d+)/, "Steam Runtime $1")}{item.downloaded_bytes!=null && <span aria-live="off"> · {(item.downloaded_bytes/1048576).toFixed(1)} MiB {pt?"recebidos":"received"}</span>}</small>
      {item.phase!=="failed" && <div className="runtime-activity-progress" aria-hidden="true"><i/></div>}
      <p>{pt ? item.phase==="failed"?"Não foi possível preparar o runtime. Verifique a conexão e tente novamente. Sem os arquivos instalados, o jogo precisa de internet para completar essa etapa.":"O jogo aguarda essa preparação. Com internet lenta, o download pode demorar. O runtime fica instalado para as próximas aberturas; novas atualizações podem exigir outro download.":item.phase==="failed"?"Runtime preparation failed. Check your connection and retry. Missing runtime files require internet access.":"The game waits for preparation. Slow connections may take longer. The runtime is kept for later launches; updates may require another download."}</p>
    </section>)}
  </aside>;
}
