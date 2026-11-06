import {useI18n} from "../i18n/I18nContext";
import {useEffect,useRef,useState} from "react";
import {voiceShortcut,voiceShortcutLabel,validateVoiceShortcuts,emptyVoiceShortcuts,type VoiceShortcutAction} from "../services/matrixVoiceShortcutKeys";
import {saveMatrixVoiceShortcuts,useMatrixShortcutPreferences} from "../services/matrixVoiceShortcuts";
export function MatrixShortcutSettings(){
  const {t: tx} = useI18n();

  const state=useMatrixShortcutPreferences();
  const [draft,setDraft]=useState(state.preferences),[recording,setRecording]=useState<VoiceShortcutAction|null>(null),[error,setError]=useState(""),[saved,setSaved]=useState(false);
  const frame=useRef<HTMLDivElement>(null);
  useEffect(()=>{
    if(!recording)return;
    const down=(event:KeyboardEvent)=>{
      event.preventDefault();event.stopImmediatePropagation();if(event.repeat||event.isComposing)return;
      if(event.code==="Escape"){setRecording(null);return;}
      const value=voiceShortcut(event);if(!value){if(!/^(Control|Alt|Shift|Meta)(Left|Right)$/.test(event.code))setError(tx("ui-matrix-shortcut-settings", "use_ctrl_alt_or_super_with_a_key_esc_cancels"));return;}
      setDraft(old=>({...old,[recording]:value}));setRecording(null);setError("");setSaved(false);
    };
    const blur=()=>setRecording(null);
    window.addEventListener("keydown",down,true);window.addEventListener("blur",blur);
    return()=>{window.removeEventListener("keydown",down,true);window.removeEventListener("blur",blur);};
  },[recording,tx]);
  const save=async()=>{const issue=validateVoiceShortcuts(draft);if(issue){setError(issue);return;}try{setSaved(false);await saveMatrixVoiceShortcuts(draft);setSaved(true);setError("");}catch(e){setError(String(e));}};
  return <div ref={frame} className="matrix-shortcut-settings" data-shortcut-recording={recording?"true":undefined}>
    <p>{tx("ui-matrix-shortcut-settings", "record_shortcuts_to_control_the_call_with_matrix_closed_for_use_while_gaming_apply_globa")}</p>
    {(["microphone","deafen"] as const).map(action=><div className="matrix-shortcut-row" key={action}><span>{action==="microphone"?(tx("ui-matrix-shortcut-settings", "toggle_microphone")):(tx("ui-matrix-shortcut-settings", "toggle_deafen"))}</span><button type="button" aria-pressed={recording===action} disabled={state.busy} onClick={()=>{setRecording(recording===action?null:action);setError("");}}>{recording===action?(tx("ui-matrix-shortcut-settings", "press_keys")):voiceShortcutLabel(draft[action])}</button><button type="button" disabled={state.busy||!draft[action]} aria-label={action === "microphone" ? tx("ui-matrix-shortcut-settings", "clear_microphone_shortcut") : tx("ui-matrix-shortcut-settings", "clear_deafen_shortcut")} onClick={()=>{setDraft(old=>({...old,[action]:""}));setSaved(false);}}>×</button></div>)}
    <div className="matrix-shortcut-actions"><button type="button" disabled={state.busy||Boolean(recording)} onClick={()=>void save()}>{state.busy?(tx("ui-matrix-shortcut-settings", "applying")):(tx("ui-matrix-shortcut-settings", "apply_shortcuts"))}</button><button type="button" disabled={state.busy} onClick={()=>{setDraft({...emptyVoiceShortcuts});setRecording(null);setSaved(false);}}>{tx("ui-matrix-shortcut-settings", "restore_defaults_no_shortcuts")}</button></div>
    {error&&<p role="alert">{error}</p>}{state.error&&<p role="alert">{state.error}</p>}
    {saved&&!state.error&&<p role="status">{"__PELI_UI_PREVIEW__" in window?(tx("ui-matrix-shortcut-settings", "shortcuts_saved_for_simulation_while_the_preview_has_focus")):state.global?(tx("ui-matrix-shortcut-settings", "global_shortcuts_active_including_while_gaming")):(tx("ui-matrix-shortcut-settings", "shortcuts_removed"))}</p>}
    {Object.entries(state.triggers).map(([action,label])=><small key={action}>{action==="microphone"?(tx("ui-matrix-shortcut-settings", "microphone")):(tx("ui-matrix-shortcut-settings", "deafen"))}: {label}</small>)}
  </div>;
}
