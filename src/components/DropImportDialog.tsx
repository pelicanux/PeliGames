import { InstallationActionIcon } from "./InstallationActionIcon";
import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "./MenuIcon";
import { SelectionMenu } from "./SelectionMenu";
import { notifyError } from "../services/notifications";
import "../modules/pelinstall/installerStyles";
import "./dropImport.css";
const EmbeddedPelinstall = lazy(() => import("../modules/pelinstall/Pelinstall").then(module => ({default:module.Pelinstall})));
type Mode = "install" | "add" | "mod";
export function DropImportDialog({ path, games, onImportMod, onLoadModGames, onClose }: {
  path: string; games: {id:string;name:string}[];
  onImportMod: (gameId:string, path:string) => Promise<boolean>; onLoadModGames: () => Promise<void>; onClose: () => void;
}) {
  const [mode,setMode] = useState<Mode|null>(null), [name,setName] = useState("");
  const [phase,setPhase] = useState<"choice"|"name"|"executables"|"assistant"|"mod">("choice");
  const [paths,setPaths] = useState<string[]>([]), [executable,setExecutable] = useState("");
  const [recommended,setRecommended] = useState<string|null>(null), [excluded,setExcluded] = useState(0);
  const [limited,setLimited] = useState(false), [busy,setBusy] = useState(false), [target,setTarget] = useState("");
  const lock=useRef(false), alive=useRef(true);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  useEffect(()=>{
    if(phase==="assistant")return;
    const key=(event:KeyboardEvent)=>{if(event.key==="Escape"){event.preventDefault();event.stopImmediatePropagation();if(!lock.current)onClose();}};
    window.addEventListener("keydown",key,true);return()=>window.removeEventListener("keydown",key,true);
  },[phase,onClose]);
  const choose=async(value:Mode)=>{
    setMode(value);setPhase(value==="mod"?"mod":"name");
    if(value!=="mod")return;
    lock.current=true;setBusy(true);
    try {await onLoadModGames();} catch(error){if(alive.current)notifyError("Biblioteca de mods",error);}
    finally {lock.current=false;if(alive.current)setBusy(false);}
  };
  const scan=async()=>{
    if(lock.current||!name.trim())return;lock.current=true;setBusy(true);
    try {
      const result = "__PELI_UI_PREVIEW__" in window && path.startsWith("/preview/")
        ? {direct_file:false,paths:[`${path}/game.exe`,`${path}/bin/game64.exe`],limited:false,recommended:mode==="add"?`${path}/game.exe`:null,excluded:mode==="add"?2:0}
        : await invoke<{direct_file:boolean;paths:string[];limited:boolean;recommended:string|null;excluded:number}>("inspect_dropped_executables",{path,name:name.trim(),mode});
      if(!alive.current)return;
      const found=result.paths.filter(file=>mode!=="add"||file.toLowerCase().endsWith(".exe"));
      if(!found.length)throw new Error("Nenhum executável principal foi encontrado. Arraste a subpasta do jogo ou o arquivo .exe diretamente para selecioná-lo manualmente.");
      setPaths(found);setLimited(result.limited);setRecommended(result.recommended);setExcluded(result.excluded);setExecutable(result.direct_file ? found[0] : "");setPhase(result.direct_file ? "assistant" : "executables");
    } catch(error){if(alive.current)notifyError("Importar arquivo",error);}
    finally {lock.current=false;if(alive.current)setBusy(false);}
  };
  const importMod=async()=>{
    if(lock.current||!target)return;lock.current=true;setBusy(true);
    try {if(await onImportMod(target,path))onClose();}
    catch(error){notifyError("Mod local",error);}
    finally {lock.current=false;if(alive.current)setBusy(false);}
  };
  return <div className="modal-overlay drop-import-overlay">
    <div className={`drop-import-dialog${phase==="assistant"?" is-assistant":""}`} role="dialog" aria-modal="true" aria-label="Importar arquivo ou pasta">
      {phase==="assistant" && mode && mode!=="mod" ? <Suspense fallback={<p role="status">Carregando Pelinstall…</p>}>
        <EmbeddedPelinstall startup={{module:"pelinstall",executable}} embedded={{mode,name:name.trim(),onClose,onBusyChange:setBusy}} />
      </Suspense> : <section className="pelinstall-window pi-window pi-embedded">
        <header className="pi-titlebar"><h1 className="pi-brand"><img src="/peligames.svg" alt=""/><span className="pelinstall-brand">Pel<span className="titlebar-games">install</span></span></h1><span className="pi-spacer"/><button type="button" className="btn-titlebar window-control pi-close" aria-label="Fechar importação" disabled={busy} onClick={onClose}>×</button></header>
        <div className="pi-body">
          <div className="pi-file"><MenuIcon name="folder"/><div className="pi-file-text"><strong>{path.split(/[\\/]/).pop()||path}</strong><small title={path}>{path}</small></div>{phase!=="choice"&&mode&&<span className="pi-chip">{mode==="add"?"Adicionar":mode==="install"?"Instalar":"Mod local"}</span>}</div>
          {phase==="choice" && <><h2>O que deseja fazer?</h2><div className="pi-cards">{([
            ["install","Instalar","Abrir um instalador Windows com o Proton.","download"],
            ["add","Adicionar","Adicionar um jogo ou programa já instalado.","plusCircle"],
            ["mod","Instalar mod","Importar um mod local para um jogo da biblioteca Nexus.","puzzle"],
          ] as const).map(([value,title,hint,icon])=><button key={value} type="button" className="pi-card" onClick={()=>void choose(value)}><InstallationActionIcon name={icon}/><span className="pi-card-copy"><strong>{title}</strong><small>{hint}</small></span></button>)}</div></>}
          {phase==="name" && <><h2>Qual o nome do jogo ou programa?</h2><div className="pi-field"><input autoFocus aria-label="Nome do jogo ou programa" placeholder="Digite o nome" maxLength={120} value={name} disabled={busy} onChange={event=>setName(event.target.value)} onKeyDown={event=>{if(event.key==="Enter")void scan();}}/></div><div className="pi-actions"><button className="pi-btn" disabled={busy} onClick={()=>setPhase("choice")}>Voltar</button><button className="pi-btn primary grow" disabled={busy||!name.trim()} onClick={()=>void scan()}>{busy?"Procurando executáveis…":"Avançar"}</button></div></>}
          {phase==="executables" && <><h2>{mode==="add"?"Qual é o executável do jogo ou programa?":"Qual instalador deseja usar?"}</h2>{recommended&&<p className="pi-hint">O executável mais provável está marcado como Recomendado. Selecione o arquivo que deseja usar.</p>}{excluded>0&&<p className="pi-hint">{excluded===1?"1 arquivo auxiliar ignorado":`${excluded} arquivos auxiliares ignorados`}, como instaladores e ferramentas de relatório de erro.</p>}<SelectionMenu label="Executável encontrado" value={executable} onChange={setExecutable} options={[{value:"",label:"Selecione um executável",disabled:true},...paths.map(value=>({value,label:`${value.slice(path.length+1)||value}${value===recommended?" · Recomendado":""}`}))]}/>{limited&&<p className="pi-hint">A busca foi limitada para evitar uma varredura extensa. Se o executável não estiver na lista, arraste a subpasta ou o arquivo diretamente.</p>}<div className="pi-actions"><button className="pi-btn" onClick={()=>setPhase("name")}>Voltar</button><button className="pi-btn primary grow" disabled={!executable} onClick={()=>setPhase("assistant")}>Avançar</button></div></>}
          {phase==="mod" && <><h2>Em qual jogo deseja instalar o mod?</h2><p className="pi-hint">O pacote será importado. Depois, revise as opções e confirme a instalação no gerenciador de mods.</p><SelectionMenu label="Jogo de destino do mod" disabled={busy} value={target} onChange={setTarget} options={[{value:"",label:"Selecione um jogo"},...games.map(game=>({value:game.id,label:game.name}))]}/>{busy&&<p className="pi-hint" role="status">Carregando…</p>}{!busy&&!games.length&&<p className="pi-hint">Adicione ou escaneie primeiro o jogo na biblioteca Nexus Mods.</p>}<div className="pi-actions"><button className="pi-btn" disabled={busy} onClick={()=>setPhase("choice")}>Voltar</button><button className="pi-btn primary grow" disabled={busy||!target} onClick={()=>void importMod()}>{busy?"Importando…":"Importar mod"}</button></div></>}
        </div>
      </section>}
    </div>
  </div>;
}
