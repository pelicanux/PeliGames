import {useState} from "react";
import {useI18n} from "../../i18n/I18nContext";
import {useDiscordPresence,saveDiscordPresence,type PresencePreferences} from "../../services/discordPresence";
import {ModOptionSwitch} from "../ModOptionRow";
export function DiscordPresencePreferences(){
 const pt=useI18n().language==="pt",presence=useDiscordPresence();
 const [busy,setBusy]=useState(false),[error,setError]=useState("");
 const save=async(patch:Partial<PresencePreferences>)=>{setBusy(true);setError("");try{await saveDiscordPresence({...presence.preferences,...patch})}catch(e){setError(String(e))}finally{setBusy(false)}};
 const options:[keyof PresencePreferences,string,string][]=[
  ["enabled",pt?"Compartilhar atividade no Discord":"Share activity on Discord",pt?"Mostra o PeliGames no seu perfil. Desligar remove a atividade publicada pelo launcher.":"Shows PeliGames on your profile. Disabling removes the activity published by the launcher."],
  ["share_game",pt?"Mostrar o jogo em execução":"Show the running game",pt?"Compartilha apenas o nome do jogo iniciado pelo PeliGames, quando estiver em execução.":"Shares only the name of a game launched through PeliGames once it is running."],
  ["share_capture",pt?"Mostrar gravação e replay":"Show recording and replay",pt?"Indica gravação de gameplay ou replay instantâneo ativo. Não compartilha vídeos nem caminhos de arquivos.":"Indicates gameplay recording or active instant replay. Does not share videos or file paths."],
 ];
 return <section className="connection-card discord-presence-card" aria-label={pt?"Atividade no Discord":"Discord activity"}>
  <h3>Discord Rich Presence</h3>
  <p className="connection-section-hint">{pt?"Usa a conta aberta no aplicativo Discord desktop. É independente do login para benefícios e não precisa de token de bot.":"Uses the account signed in to the Discord desktop app. Independent of benefit verification; no bot token required."}</p>
  {options.map(([key,label,description])=><div className="mod-setting-row" key={key}><div className="mod-row-copy"><strong>{label}</strong><small>{description}</small></div><ModOptionSwitch label={label} enabled={presence.preferences[key]} disabled={busy||!presence.ready||(key!=="enabled"&&!presence.preferences.enabled)} onToggle={()=>void save({[key]:!presence.preferences[key]})}/></div>)}
  <p role="status" className="discord-presence-status">{!presence.ready?(pt?"Carregando preferências…":"Loading preferences…"):!presence.preferences.enabled?(pt?"Compartilhamento desligado.":"Activity sharing is off."):presence.connected?(pt?"Conectado ao Discord desktop.":"Connected to Discord desktop."):(pt?"Aguardando o Discord desktop. A reconexão é automática.":"Waiting for Discord desktop. Reconnects automatically.")}</p>
  {presence.preferences.enabled&&<p className="connection-section-hint">{pt?"No Discord, habilite o compartilhamento de atividade nas configurações de privacidade. A forma de exibição depende das preferências do Discord.":"Enable activity sharing in Discord's privacy settings. Display depends on your Discord preferences."}</p>}
  {(error||presence.error)&&<p role="alert" className="mod-feedback-error">{error||presence.error}</p>}
  {"__PELI_UI_PREVIEW__" in window&&<p className="connection-section-hint">{pt?"Prévia: conexão simulada. Nenhuma atividade é publicada no Discord.":"Preview: simulated connection. No activity is published to Discord."}</p>}
 </section>;
}
