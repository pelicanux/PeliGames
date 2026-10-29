import {useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {openDirectoryPicker} from "../services/tauriService";
import {NexusPopup} from "../modules/nexus/NexusPopup";
import {MenuIcon} from "./MenuIcon";
import {notifyInfo} from "../services/notifications";
import "./GameCopyDialog.css";
import "./LibraryRecoveryDialog.css";
interface Candidate{directory:string;name:string;kind:string;games:number;registered:boolean;warnings:string[];error:string|null}
interface Discovery{token:string;candidates:Candidate[];warnings:string[]}
export function LibraryRecoveryDialog({onClose}:{onClose:()=>void}){
 const [roots,setRoots]=useState<string[]>([]),[result,setResult]=useState<Discovery>(),[selected,setSelected]=useState<string[]>([]),[busy,setBusy]=useState(false),[error,setError]=useState("");
 const demo="__PELI_UI_PREVIEW__" in window;
 const search=async(next:string[])=>{setBusy(true);setError("");setResult(undefined);setSelected([]);try{
  const found=demo?{token:"preview",warnings:[],candidates:[{directory:`${next[0]}/Real MU`,name:"Real MU",kind:"Instalação",games:1,registered:false,warnings:[],error:null},{directory:`${next[0]}/Real MU (1)`,name:"Real MU (1)",kind:"Overlay",games:1,registered:false,warnings:[],error:null}]}:await invoke<Discovery>("discover_peligames_library",{roots:next});
  setResult(found);setSelected(found.candidates.filter(c=>!c.error&&!c.registered).map(c=>c.directory));
 }catch(e){setError(String(e));}finally{setBusy(false);}};
 const add=async()=>{try{const path=await openDirectoryPicker("Localizar biblioteca PeliGames");if(path){const next=[...new Set([...roots,path])];setRoots(next);await search(next);}}catch(e){setError(String(e));}};
 const restore=async()=>{if(busy)return;
  if(!result||!selected.length)return;if(demo){setError("A prévia não importa instalações reais.");return;}setBusy(true);setError("");try{const imported=await invoke<{installations:number;games:number}>("restore_peligames_library",{token:result.token,selected});window.dispatchEvent(new Event("peligamesLibraryChanged"));notifyInfo("Biblioteca recuperada",`${imported.installations} instalações e ${imported.games} jogos adicionados.`);onClose();}catch(e){setError(String(e));}finally{setBusy(false);}};
 return <NexusPopup className="game-copy-dialog library-recovery-dialog" title="Localizar biblioteca existente" titleIcon={<MenuIcon name="folder"/>} onClose={()=>{if(!busy)onClose();}} footer={<><button type="button" className="btn btn-secondary" disabled={busy} onClick={onClose}>Cancelar</button><button type="button" className="btn btn-primary" disabled={busy||!result||!selected.length} onClick={()=>void restore()}>{busy?"Aguarde…":`Importar selecionados${selected.length?` (${selected.length})`:""}`}</button></>}>
  <div className="game-copy-form">
   <p>Recupere jogos, programas e cópias já instalados pelo PeliGames, usando os registros que ficam nas pastas.</p>
   <div className="library-recovery-actions"><button type="button" className="modern-mini-btn" disabled={busy||roots.length>=8} onClick={()=>void add()}><MenuIcon name="folder"/>{roots.length?"Incluir outra pasta":"Selecionar pasta"}</button>{roots.length>0&&<button type="button" className="modern-mini-btn" disabled={busy} onClick={()=>void search(roots)}>Buscar novamente</button>}</div>
   {roots.length>0&&<ul className="library-recovery-roots">{roots.map(root=><li key={root} title={root}><span>{root}</span><button type="button" className="modern-mini-btn" disabled={busy} aria-label={`Retirar ${root} da busca`} onClick={()=>{setRoots(roots.filter(r=>r!==root));setResult(undefined);setSelected([]);}}>×</button></li>)}</ul>}
   <small>Para overlays, inclua a base original e as cópias, mesmo que estejam em discos diferentes. Reflinks e cópias completas não precisam da origem. A busca não monta overlays nem abre executáveis.</small>
   {busy&&<p role="status">Buscando jogos… <progress aria-label="Recuperação da biblioteca em andamento"/></p>}
   {result&&<><div className="library-recovery-results" role="group" aria-label="Instalações encontradas">{result.candidates.length?result.candidates.map(c=><label key={c.directory} className="library-recovery-item"><input type="checkbox" disabled={busy||c.registered||Boolean(c.error)} checked={selected.includes(c.directory)} onChange={e=>setSelected(previous=>e.target.checked?[...previous,c.directory]:previous.filter(p=>p!==c.directory))}/><span><strong>{c.name}</strong><small>{c.kind} · {c.games} jogo(s){c.registered?" · Já na biblioteca":""}</small><small className="library-recovery-path" title={c.directory}>{c.directory}</small>{c.warnings.map(w=><small key={w}>{w}</small>)}{c.error&&<small className="game-copy-error">{c.error}</small>}</span></label>):<p>Nenhum registro PeliGames encontrado. Selecione a pasta que contém os jogos ou uma instalação com o arquivo installation.json.</p>}</div>{result.warnings.map(w=><p key={w} className="game-copy-warning">{w}</p>)}</>}
   <small>São restaurados os registros locais. Proton, wrappers e integrações da instalação anterior podem precisar ser configurados novamente. Esta opção não substitui o backup dos jogos e dos saves.</small>
   {error&&<p role="alert" className="game-copy-error">{error}</p>}
  </div>
 </NexusPopup>;
}
