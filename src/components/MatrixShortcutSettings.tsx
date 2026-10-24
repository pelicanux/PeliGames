import {useEffect,useRef,useState} from "react";
import {voiceShortcut,voiceShortcutLabel,validateVoiceShortcuts,emptyVoiceShortcuts,type VoiceShortcutAction} from "../services/matrixVoiceShortcutKeys";
import {saveMatrixVoiceShortcuts,useMatrixShortcutPreferences} from "../services/matrixVoiceShortcuts";
export function MatrixShortcutSettings({pt}:{pt:boolean}){
  const state=useMatrixShortcutPreferences();
  const [draft,setDraft]=useState(state.preferences),[recording,setRecording]=useState<VoiceShortcutAction|null>(null),[error,setError]=useState(""),[saved,setSaved]=useState(false);
  const frame=useRef<HTMLDivElement>(null);
  useEffect(()=>{
    if(!recording)return;
    const down=(event:KeyboardEvent)=>{
      event.preventDefault();event.stopImmediatePropagation();if(event.repeat||event.isComposing)return;
      if(event.code==="Escape"){setRecording(null);return;}
      const value=voiceShortcut(event);if(!value){if(!/^(Control|Alt|Shift|Meta)(Left|Right)$/.test(event.code))setError(pt?"Use Ctrl, Alt ou Super com uma tecla; Esc cancela.":"Use Ctrl, Alt or Super with a key; Esc cancels.");return;}
      setDraft(old=>({...old,[recording]:value}));setRecording(null);setError("");setSaved(false);
    };
    const blur=()=>setRecording(null);
    window.addEventListener("keydown",down,true);window.addEventListener("blur",blur);
    return()=>{window.removeEventListener("keydown",down,true);window.removeEventListener("blur",blur);};
  },[recording,pt]);
  const save=async()=>{const issue=validateVoiceShortcuts(draft);if(issue){setError(issue);return;}try{setSaved(false);await saveMatrixVoiceShortcuts(draft);setSaved(true);setError("");}catch(e){setError(String(e));}};
  return <div ref={frame} className="matrix-shortcut-settings" data-shortcut-recording={recording?"true":undefined}>
    <p>{pt?"Grave combinações para controlar a chamada com o Matrix fechado. Para usar durante o jogo, aplique os atalhos globais e confirme a janela do sistema, se aparecer.":"Record shortcuts to control the call with Matrix closed. For use while gaming, apply global shortcuts and confirm the system dialog if shown."}</p>
    {(["microphone","deafen"] as const).map(action=><div className="matrix-shortcut-row" key={action}><span>{action==="microphone"?(pt?"Mutar / desmutar microfone":"Toggle microphone"):(pt?"Silenciar / reativar áudio e microfone":"Toggle deafen")}</span><button type="button" aria-pressed={recording===action} disabled={state.busy} onClick={()=>{setRecording(recording===action?null:action);setError("");}}>{recording===action?(pt?"Pressione as teclas…":"Press keys…"):voiceShortcutLabel(draft[action])}</button><button type="button" disabled={state.busy||!draft[action]} aria-label={pt?`Limpar atalho de ${action==="microphone"?"microfone":"silenciamento geral"}`:`Clear ${action} shortcut`} onClick={()=>{setDraft(old=>({...old,[action]:""}));setSaved(false);}}>×</button></div>)}
    <div className="matrix-shortcut-actions"><button type="button" disabled={state.busy||Boolean(recording)} onClick={()=>void save()}>{state.busy?(pt?"Aplicando…":"Applying…"):(pt?"Aplicar atalhos":"Apply shortcuts")}</button><button type="button" disabled={state.busy} onClick={()=>{setDraft({...emptyVoiceShortcuts});setRecording(null);setSaved(false);}}>{pt?"Restaurar padrão (sem atalhos)":"Restore defaults (no shortcuts)"}</button></div>
    {error&&<p role="alert">{error}</p>}{state.error&&<p role="alert">{state.error}</p>}
    {saved&&!state.error&&<p role="status">{"__PELI_UI_PREVIEW__" in window?(pt?"Atalhos salvos para simulação com a prévia em foco.":"Shortcuts saved for simulation while the preview has focus."):state.global?(pt?"Atalhos globais ativos, inclusive com o jogo em foco.":"Global shortcuts active, including while gaming."):(pt?"Atalhos removidos.":"Shortcuts removed.")}</p>}
    {Object.entries(state.triggers).map(([action,label])=><small key={action}>{action==="microphone"?(pt?"Microfone":"Microphone"):(pt?"Silenciamento geral":"Deafen")}: {label}</small>)}
  </div>;
}
