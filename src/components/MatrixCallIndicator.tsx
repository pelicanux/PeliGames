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
  const pt=useI18n().language==="pt",voice=useMatrixVoice(),chat=useMatrixChat();
  useMatrixVoiceShortcuts();
  if(voice.phase==="idle"||!voice.room)return null;
  const room=chat.roomList.find(room=>room.room_id===voice.room)?.name || (pt?"Chamada Matrix":"Matrix call");
  const status=voice.phase==="connected"?(pt?"Conectado":"Connected"):voice.phase==="reconnecting"?(pt?"Reconectando…":"Reconnecting…"):voice.phase==="leaving"?(pt?"Saindo…":"Leaving…"):(pt?"Conectando…":"Connecting…");
  const busy=voice.phase==="joining"||voice.phase==="leaving";
  const action=(run:()=>Promise<void>)=>void run().catch(error=>notifyError("Matrix",String(error)));
  return <section className="matrix-call-indicator" data-phase={voice.phase} data-tauri-drag-region="false" aria-label={pt?"Chamada Matrix ativa":"Active Matrix call"}>
    <button type="button" className="matrix-call-room" title={`${status} · ${room}`} aria-label={`${status} · ${room}`} onClick={()=>{requestCallView();onOpen();}}><span className="matrix-call-dot"/><span className="matrix-call-room-copy"><strong>{room}</strong><small role="status">{status}{"__PELI_UI_PREVIEW__" in window?(pt?" · simulação":" · simulated"):""}</small></span></button>
    <button type="button" className={voice.muted||voice.deafened?"is-muted":""} disabled={busy} aria-pressed={voice.muted||voice.deafened} title={pt?"Alternar microfone":"Toggle microphone"} aria-label={pt?"Alternar microfone":"Toggle microphone"} onClick={()=>action(toggleMatrixMicrophone)}><MicIcon off={voice.muted||voice.deafened}/></button>
    <button type="button" className={voice.deafened?"is-muted":""} disabled={busy} aria-pressed={voice.deafened} title={pt?"Alternar silenciamento geral":"Toggle deafen"} aria-label={pt?"Alternar silenciamento geral":"Toggle deafen"} onClick={()=>action(toggleMatrixDeafen)}><HeadphonesIcon off={voice.deafened}/></button>
    <button type="button" className="matrix-call-disconnect" disabled={voice.phase==="leaving"} title={pt?"Desconectar da chamada":"Disconnect call"} aria-label={pt?"Desconectar da chamada":"Disconnect call"} onClick={()=>action(()=>leaveMatrixVoice())}><LeaveIcon/></button>
  </section>;
}
