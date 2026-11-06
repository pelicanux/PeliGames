import {useI18n} from "../i18n/I18nContext";
import {useMatrixChat} from "../services/matrixChat";
import {requestCallView} from "../services/matrixScreenView";
import {leaveMatrixVoice,toggleMatrixMicrophone,toggleMatrixDeafen,useMatrixVoice} from "../services/matrixVoice";
import {useMatrixVoiceShortcuts} from "../services/matrixVoiceShortcuts";
import {notifyError} from "../services/notifications";
import {MicIcon,HeadphonesIcon} from "./MatrixUserPanel";
import {LeaveIcon} from "./MatrixCallStage";
import "./MatrixCallIndicator.css";

export function MatrixCallIndicator({onOpen}:{onOpen:()=>void}){
  const {t: tx} = useI18n();

  const voice=useMatrixVoice(),chat=useMatrixChat();
  useMatrixVoiceShortcuts();
  if(voice.phase==="idle"||!voice.room)return null;
  const room=chat.roomList.find(room=>room.room_id===voice.room)?.name || (tx("ui-matrix-call-indicator", "matrix_call"));
  const status=voice.phase==="connected"?(tx("ui-matrix-call-indicator", "connected")):voice.phase==="reconnecting"?(tx("ui-matrix-call-indicator", "reconnecting")):voice.phase==="leaving"?(tx("ui-matrix-call-indicator", "leaving")):(tx("ui-matrix-call-indicator", "connecting"));
  const busy=voice.phase==="joining"||voice.phase==="leaving";
  const action=(run:()=>Promise<void>)=>void run().catch(error=>notifyError("Matrix",String(error)));
  return <section className="matrix-call-indicator" data-phase={voice.phase} data-tauri-drag-region="false" aria-label={tx("ui-matrix-call-indicator", "active_matrix_call")}>
    <button type="button" className="matrix-call-room" title={`${status} · ${room}`} aria-label={`${status} · ${room}`} onClick={()=>{requestCallView();onOpen();}}><span className="matrix-call-dot"/><span className="matrix-call-room-copy"><strong>{room}</strong><small role="status">{status}{"__PELI_UI_PREVIEW__" in window?(tx("ui-matrix-call-indicator", "simulated")):""}</small></span></button>
    <button type="button" className={voice.muted||voice.deafened?"is-muted":""} disabled={busy} aria-pressed={voice.muted||voice.deafened} title={tx("ui-matrix-call-indicator", "toggle_microphone")} aria-label={tx("ui-matrix-call-indicator", "toggle_microphone")} onClick={()=>action(toggleMatrixMicrophone)}><MicIcon off={voice.muted||voice.deafened}/></button>
    <button type="button" className={voice.deafened?"is-muted":""} disabled={busy} aria-pressed={voice.deafened} title={tx("ui-matrix-call-indicator", "toggle_deafen")} aria-label={tx("ui-matrix-call-indicator", "toggle_deafen")} onClick={()=>action(toggleMatrixDeafen)}><HeadphonesIcon off={voice.deafened}/></button>
    <button type="button" className="matrix-call-disconnect" disabled={voice.phase==="leaving"} title={tx("ui-matrix-call-indicator", "disconnect_call")} aria-label={tx("ui-matrix-call-indicator", "disconnect_call")} onClick={()=>action(()=>leaveMatrixVoice())}><LeaveIcon/></button>
  </section>;
}
