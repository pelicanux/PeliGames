import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {MangoHudShortcutRecorder} from "../../components/preferences/MangoHudShortcutRecorder";
import {normalizedShortcut} from "../../components/preferences/shortcutConflicts";
import "../frameGeneration/frameOverlay.css";
type Preferences={recording:string;replay:string;save_replay:string};
const defaults:Preferences={recording:"Alt_L+F6",replay:"Alt_L+F7",save_replay:"Alt_L+F8"};
const actions=[['recording','Iniciar ou parar gravação'],['replay','Ligar ou desligar replay'],['save_replay','Salvar clipe do replay']] as const;
const physical=(v:string)=>normalizedShortcut(v.replace(/_(L|R)(?=\+|$)/g,'_L'));
export function CaptureShortcuts(){
 const [value,setValue]=useState<Preferences>(),[busy,setBusy]=useState(false),[error,setError]=useState('');
 useEffect(()=>{let alive=true;void invoke<Preferences>('capture_shortcut_preferences').then(v=>{if(alive)setValue(v)}).catch(e=>{if(alive)setError(String(e))});return()=>{alive=false};},[]);
 const save=async(key:keyof Preferences,shortcut:string)=>{
  if(!value||busy)return;setError('');
  const next={...value,[key]:shortcut};
  if(shortcut&&actions.some(([other])=>other!==key&&next[other]&&physical(next[other])===physical(shortcut))){setError('Essa combinação já está atribuída a outra ação de captura.');return;}
  setBusy(true);try{await invoke('configure_capture_shortcuts',{preferences:next});setValue(next)}catch(e){setError(String(e))}finally{setBusy(false)}
 };
 return <section className="modern-performance-card capture-shortcuts" aria-label="Atalhos de captura"><h3>Atalhos de captura</h3><p>Valem para todos os jogos. Ative o Overlay PeliGames na aba Desempenho; os atalhos funcionam no jogo mesmo com o painel fechado. Reabra o jogo após mudar as teclas.</p>
 {value&&actions.map(([key,label])=><div key={key} className="fg-overlay-settings fg-overlay-shortcut-row"><div><strong>{label}</strong><small>{key==='replay'?'Encerre a gravação antes de desligar o replay.':key==='save_replay'?'Salva os últimos momentos com o replay ativo.':'Para e salva o vídeo; mantém o replay quando estiver ligado.'}</small></div><fieldset disabled={busy} className="mm-shortcut-actions fg-overlay-shortcut-actions"><MangoHudShortcutRecorder label={label} value={value[key]||'Não atribuído'} onChange={v=>void save(key,v)}/><button type="button" className="mm-ghost" disabled={!value[key]} onClick={()=>void save(key,'')}>Remover</button><button type="button" className="mm-ghost capture-shortcut-reset" disabled={physical(value[key])===physical(defaults[key])} onClick={()=>void save(key,defaults[key])}>Padrão</button></fieldset></div>)}
 {error&&<p role="alert" className="capture-error">{error}</p>}<small>As teclas só atuam com o jogo em foco; não são atalhos globais do desktop.</small></section>;
}
