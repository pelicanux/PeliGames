import {useState} from "react";
import {useI18n} from "../../i18n/I18nContext";
import {useDiscordPresence,saveDiscordPresence,type PresencePreferences} from "../../services/discordPresence";
import {ModOptionSwitch} from "../ModOptionRow";
export function DiscordPresencePreferences(){
 const pt=useI18n().language==="pt",presence=useDiscordPresence();
 const [busy,setBusy]=useState(false),[error,setError]=useState("");
 const save=async(patch:Partial<PresencePreferences>)=>{setBusy(true);setError("");try{await saveDiscordPresence({...presence.preferences,...patch})}catch(e){setError(String(e))}finally{setBusy(false)}};
 return <section className="connection-card discord-presence-card" aria-label={pt?"Atividade no Discord":"Discord activity"}>
  <h3>Discord Rich Presence</h3>
  <p className="connection-section-hint">{pt?"Mostra uma atividade do PeliGames somente enquanto grava a tela ou mantém o replay instantâneo ativo. Não publica a biblioteca nem o nome do jogo.":"Shows a PeliGames activity only while recording the screen or running instant replay. Does not publish library activity or game names."}</p>
  <div className="mod-setting-row"><div className="mod-row-copy"><strong>{pt?"Mostrar gravação e replay no Discord":"Show recording and replay on Discord"}</strong><small>{pt?"A atividade desaparece ao encerrar a captura. Vídeos, áudio e caminhos de arquivos não são enviados.":"The activity disappears when capture ends. Video, audio and file paths are not sent."}</small></div><ModOptionSwitch label={pt?"Mostrar gravação e replay no Discord":"Show recording and replay on Discord"} enabled={presence.preferences.enabled} disabled={busy||!presence.ready} onToggle={()=>void save({enabled:!presence.preferences.enabled})}/></div>
  <p role="status" className="discord-presence-status">{!presence.ready?(pt?"Carregando preferências…":"Loading preferences…"):!presence.preferences.enabled?(pt?"Compartilhamento desligado.":"Activity sharing is off."):!presence.active?(pt?"Aguardando gravação ou replay. Nenhuma atividade publicada.":"Waiting for recording or replay. No activity published."):presence.connected?(pt?"Gravação/replay compartilhado no Discord.":"Recording/replay shared on Discord."):(pt?"Aguardando o Discord desktop. A reconexão é automática.":"Waiting for Discord desktop. Reconnects automatically.")}</p>
  {presence.preferences.enabled&&<p className="connection-section-hint">{pt?"No Discord, habilite o compartilhamento de atividade nas configurações de privacidade. A forma de exibição depende das preferências do Discord.":"Enable activity sharing in Discord's privacy settings. Display depends on your Discord preferences."}</p>}
  {(error||presence.error)&&<p role="alert" className="mod-feedback-error">{error||presence.error}</p>}
  {"__PELI_UI_PREVIEW__" in window&&<p className="connection-section-hint">{pt?"Prévia: conexão simulada. Nenhuma atividade é publicada no Discord.":"Preview: simulated connection. No activity is published to Discord."}</p>}
 </section>;
}
