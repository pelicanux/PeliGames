import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {open} from "@tauri-apps/plugin-dialog";
import {NexusPopup} from "../modules/nexus/NexusPopup";
import {MenuIcon} from "./MenuIcon";
import type {GameInfo} from "./GameGrid";
import {notifyInfo} from "../services/notifications";
import "./GameCopyDialog.css";
type Kind="full"|"reflink"|"overlay";
interface Options{parent:string;filesystem:string;reflink:boolean;overlay:boolean;shared_game_directory?:string|null}
export function GameCopyDialog({game,onClose,onBusy}:{game:GameInfo;onClose:()=>void;onBusy:(busy:boolean)=>void}){
 const [name,setName]=useState(`${game.name} - cópia`),[parent,setParent]=useState(""),[options,setOptions]=useState<Options|null>(null),[kind,setKind]=useState<Kind>("full");
 const [busy,setBusy]=useState(false),[error,setError]=useState(""),[message,setMessage]=useState(""),[cancelling,setCancelling]=useState(false);
 const [acknowledged,setAcknowledged]=useState(false);
 const request=useRef<string|null>(null),checking=useRef(0),mounted=useRef(true);
 const demo="__PELI_UI_PREVIEW__" in window;
 const load=async(folder?:string)=>{const revision=++checking.current;setOptions(null);setError("");try{
  const next:Options=demo?{parent:folder||"/preview/Games/PeliGames",filesystem:"ext4 (demonstração)",reflink:false,overlay:true}:await invoke("game_copy_options",{path:game.path,parent:folder||null});
  if(mounted.current&&checking.current===revision){setParent(next.parent);setOptions(next);if(kind!=="full")setKind(next.reflink?"reflink":next.overlay?"overlay":"full");}
 }catch(e){if(mounted.current&&checking.current===revision)setError(String(e));}};
 useEffect(()=>{mounted.current=true;void load();return()=>{mounted.current=false;};},[game.path]);
 const choose=async()=>{try{const folder=await open({directory:true,multiple:false,title:"Pasta para a cópia",defaultPath:parent||undefined});if(typeof folder==="string")await load(folder);}catch(e){setError(String(e));}};
 const start=async()=>{
  if(busy||!options||!name.trim()||!acknowledged)return;
  if(demo){setError("A prévia mostra as opções, mas não copia arquivos reais.");return;}
  const id=crypto.randomUUID();request.current=id;setBusy(true);onBusy(true);setError("");setMessage("Preparando a cópia…");
  let stop:(()=>void)|undefined;
  try{stop=await listen<{id:string;message:string}>("peligames-copy-progress",event=>{if(event.payload.id===id&&mounted.current)setMessage(event.payload.message);});
   await invoke("duplicate_peligames_game",{request:{path:game.path,name:name.trim(),parent,kind,acknowledged},requestId:id});
   window.dispatchEvent(new Event("peligamesLibraryChanged"));notifyInfo("Cópia do jogo",`${name.trim()} foi adicionada à biblioteca.`);onClose();
  }catch(e){if(mounted.current)setError(String(e));}
  finally{stop?.();request.current=null;onBusy(false);if(mounted.current){setBusy(false);setCancelling(false);setMessage("");}}
 };
 const cancel=async()=>{if(!request.current||cancelling)return;setCancelling(true);try{await invoke("cancel_game_copy",{requestId:request.current});}catch(e){setError(String(e));setCancelling(false);}};
 const external=Boolean(options?.shared_game_directory);
 const economic:Kind|null=options?.reflink?"reflink":options?.overlay?"overlay":null;
 return <NexusPopup className="game-copy-dialog" title={`Criar cópia · ${game.name}`} titleIcon={<MenuIcon name="copy"/>} onClose={()=>{if(!busy)onClose();}} footer={<>
  <button type="button" className="btn btn-secondary" disabled={cancelling} onClick={()=>busy?void cancel():onClose()}>{busy?(cancelling?"Cancelando…":"Cancelar cópia"):"Cancelar"}</button>
  <button type="button" className="btn btn-primary" disabled={busy||!options||!name.trim()||!acknowledged} onClick={()=>void start()}><MenuIcon name="copy"/>{busy?"Copiando…":"Criar cópia"}</button>
 </>}>
  <div className="game-copy-form">
   <label>Nome da cópia<input value={name} maxLength={100} disabled={busy} onChange={e=>setName(e.target.value)}/></label>
   <div className="game-copy-destination"><span>Salvar em<code>{parent?`${parent}/${name.trim()}`:"Verificando destino…"}</code></span><button type="button" className="modern-mini-btn" disabled={busy||!options} onClick={()=>{setAcknowledged(false);void choose();}}><MenuIcon name="folder"/>Alterar pasta</button></div>
   {external&&<div className="game-copy-explanation"><strong>Copiar apenas o prefixo</strong><p>O jogo está em outra pasta. Cada cópia terá seu próprio ambiente Windows e usará o mesmo executável. Os arquivos da pasta do jogo, inclusive configurações e logs salvos nela, serão compartilhados.</p><code>{options?.shared_game_directory}</code></div>}
   {options&&<><div role="radiogroup" aria-label="Tipo de cópia" className="game-copy-methods">
    <button type="button" role="radio" aria-checked={kind==="full"} disabled={busy} onClick={()=>{setKind("full");setAcknowledged(false);}}><strong>{external?"Cópia completa do prefixo":"Cópia completa"}</strong><span>{external?"Copia todos os arquivos do prefixo. A pasta externa do jogo não será copiada.":"Jogo e prefixo independentes. Ocupa o espaço de uma cópia completa."}</span></button>
    {economic&&<button type="button" role="radio" aria-checked={kind===economic} disabled={busy} onClick={()=>{setKind(economic);setAcknowledged(false);}}><strong>{economic==="reflink"?"Cópia com reflink":"Cópia com overlay"}</strong><span>{economic==="reflink"?"Prefixo independente, compartilha espaço até os arquivos serem alterados.":"Economiza espaço. Depende da instalação original no mesmo local. Não mova nem apague a pasta original."}</span></button>}
   </div><small>{external?"Destino do prefixo":"Destino"}: {options.filesystem}{!economic&&" · Sem suporte a cópia econômica"}</small></>}
   <div className="game-copy-explanation">
    <p>{external?(kind==="overlay"?"O prefixo original ficará reservado como base. As alterações de cada prefixo serão separadas, mas a pasta externa do jogo será compartilhada. Não mova nem apague a base ou a pasta do jogo.":"O novo prefixo será independente do original. Ambos usarão a mesma pasta externa do jogo: alterações feitas nessa pasta serão vistas por todas as instâncias. Apagar a cópia pelo PeliGames não apaga o jogo externo."):kind==="overlay"?"A instalação original ficará reservada como base, sem poder ser executada ou alterada pelo PeliGames. Cada cópia guarda suas próprias alterações; removê-la não afeta as demais. Ao remover o último overlay, a base volta a ficar disponível.":kind==="reflink"?"A cópia será independente e compartilhará espaço até os arquivos serem alterados. A instalação original continuará disponível e poderá ser removida sem afetar a cópia.":"O jogo e o prefixo serão copiados por completo. A cópia será independente e a instalação original continuará disponível."}</p>
    {kind==="overlay"&&<p>A pasta original será a base das cópias. Não mova, apague nem altere essa pasta fora do PeliGames.</p>}
    <label className="game-copy-acknowledgement"><input type="checkbox" checked={acknowledged} disabled={busy||!options} onChange={event=>setAcknowledged(event.target.checked)}/>Li e entendi como esta cópia funciona.</label>
   </div>
   <details className="game-copy-info"><summary>Contas e configurações</summary><p>As informações locais de login existentes serão copiadas. Use uma conta diferente em cada instância, se necessário. Configure novamente comandos de inicialização e wrappers personalizados. Arquivos em pastas externas ao prefixo podem continuar compartilhados.</p></details>
   {!options&&!error&&<p role="status">Verificando o disco e os métodos disponíveis…</p>}
   {busy&&<div role="status" className="game-copy-progress">{cancelling?"Cancelando e removendo a cópia incompleta…":message}<progress aria-label="Cópia em andamento"/></div>}
   {error&&<p role="alert" className="game-copy-error">{error}{!options&&<button type="button" className="modern-mini-btn" onClick={()=>void load()}>Tentar novamente</button>}</p>}
  </div>
 </NexusPopup>;
}
