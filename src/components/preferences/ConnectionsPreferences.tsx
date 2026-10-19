import { useI18n } from "../../i18n/I18nContext";
import { NexusAccountPanel } from "../../modules/nexus/NexusAccountPanel";
import { usePreferencesText } from "./labels";
import { DiscordConnection } from "./DiscordConnection";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../MenuIcon";
import { notifyError, notifyInfo } from "../../services/notifications";
import { MatrixChatPanel } from "../MatrixChatPanel";
import { MatrixIcon } from "../MatrixIcon";
import { MatrixNotificationSoundPreferences } from "./MatrixNotificationSoundPreferences";
import { displayName, refreshMatrixStatus, useMatrixChat } from "../../services/matrixChat";
import { setMatrixActivityPreferences, useMatrixActivityPreferences } from "../../services/matrixGameActivity";

const CONNECTION_TABS=[{id:"nexus",label:"Nexus Mods"},{id:"discord",label:"Discord"},{id:"matrix",label:"Matrix"}] as const;
type ConnectionTab=typeof CONNECTION_TABS[number]["id"];
export function ConnectionsPreferences({searchMode=false,focusTarget}:{searchMode?:boolean;focusTarget?:string|null}) {
  const { language } = useI18n();
  const text = usePreferencesText();
  const pt = language === "pt";
  const matrix = useMatrixChat();
  const activity = useMatrixActivityPreferences();
  const [tab,setTab]=useState<ConnectionTab>("nexus");
  useEffect(()=>{const id=focusTarget?.replace("connection-","");if(CONNECTION_TABS.some(item=>item.id===id))setTab(id as ConnectionTab)},[focusTarget]);
  const [matrixOpen, setMatrixOpen] = useState(false);
  const [removingMatrix, setRemovingMatrix] = useState(false);
  const removalLock = useRef(false);
  const removeMatrix = async () => {
    if (removalLock.current) return;
    removalLock.current = true; setRemovingMatrix(true);
    try {
      const revoked = await invoke<boolean>("disconnect_matrix_chat");
      await refreshMatrixStatus();
      notifyInfo("Matrix", revoked
        ? (pt ? "Conexão removida e credenciais apagadas deste dispositivo." : "Connection removed and credentials deleted from this device.")
        : (pt ? "Credenciais removidas deste dispositivo. Sem confirmação do servidor; encerre esta sessão também no Element, em Configurações → Sessões." : "Credentials removed from this device. The server could not confirm sign-out; also end this session in Element under Settings → Sessions."));
    } catch (error) { notifyError("Matrix", String(error)); }
    finally { removalLock.current = false; setRemovingMatrix(false); }
  };
  return <div className="connections-workspace mod-workspace">
    {matrixOpen && <MatrixChatPanel onClose={() => setMatrixOpen(false)}/>}
    {!searchMode&&<><header className="mod-intro"><h3>{pt?"Conecte os serviços que você usa":"Connect the services you use"}</h3><p>{pt?"Vincule uma conta, confira o acesso e escolha o que compartilhar.":"Link an account, check access and choose what to share."}</p></header>
    <div className="preference-mod-tabs connection-tabs" role="tablist" aria-label={text.connections} data-active={tab} onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;event.preventDefault();const index=CONNECTION_TABS.findIndex(item=>item.id===tab);const next=CONNECTION_TABS[event.key==="Home"?0:event.key==="End"?2:(index+(event.key==="ArrowLeft"?-1:1)+3)%3].id;setTab(next);requestAnimationFrame(()=>document.getElementById(`connection-tab-${next}`)?.focus())}}>{CONNECTION_TABS.map(item=><button type="button" key={item.id} role="tab" id={`connection-tab-${item.id}`} aria-selected={tab===item.id} aria-controls={`connection-panel-${item.id}`} tabIndex={tab===item.id?0:-1} onClick={()=>setTab(item.id)}>{item.label}</button>)}</div></>}
    <div className="connection-tab-panel" hidden={!searchMode&&tab!=="nexus"} role={searchMode?undefined:"tabpanel"} id="connection-panel-nexus" aria-labelledby={searchMode?undefined:"connection-tab-nexus"}><section className="connection-module" data-preference-id="connection-nexus"><NexusAccountPanel/></section></div>
    <div className="connection-tab-panel" hidden={!searchMode&&tab!=="discord"} role={searchMode?undefined:"tabpanel"} id="connection-panel-discord" aria-labelledby={searchMode?undefined:"connection-tab-discord"}><DiscordConnection/></div>
    <div className="connection-tab-panel" hidden={!searchMode&&tab!=="matrix"} role={searchMode?undefined:"tabpanel"} id="connection-panel-matrix" aria-labelledby={searchMode?undefined:"connection-tab-matrix"}>
      <section className="connection-module" data-preference-id="connection-matrix"><div className="connection-grid">
        <section className="connection-card"><header className="connection-heading"><span className="mod-component-icon"><MatrixIcon/></span><div><h3>Matrix</h3><p>{pt?"Converse com a comunidade pelo launcher.":"Chat with the community in the launcher."}</p></div><span className="mod-status-badge" data-ready={Boolean(matrix.userId)} role="status">{matrix.userId?(pt?"Conectado":"Connected"):matrix.pending?(pt?"Aguardando login":"Waiting for sign-in"):(pt?"Não conectado":"Not connected")}</span></header>
        <div className="connection-account-row"><MenuIcon name={matrix.userId?"check":"chainLinked"}/><div><strong>{matrix.userId?displayName(matrix.userId,matrix.names):(pt?"Entre na sua conta":"Sign in to your account")}</strong><p>{matrix.userId?matrix.userId:(pt?"Abra o chat para autorizar a conexão pelo navegador.":"Open the chat to authorize the connection in your browser.")}</p></div></div>
        <div className="connection-primary-actions"><button type="button" className="mod-action-primary" disabled={removingMatrix} onClick={()=>setMatrixOpen(true)}><MatrixIcon/>{pt?"Abrir chat Matrix":"Open Matrix chat"}</button></div>
        <div className="mod-setting-row mod-destructive-row"><div className="mod-row-copy"><strong>{pt?"Remover conexão":"Remove connection"}</strong><small>{pt?"Apaga as credenciais deste dispositivo. Sua conta continua existindo.":"Deletes credentials from this device. Your account is preserved."}</small></div><button type="button" className="mod-action-quiet danger" disabled={removingMatrix||matrix.pending} aria-busy={removingMatrix} onClick={()=>void removeMatrix()}><MenuIcon name="chainBroken"/>{removingMatrix?(pt?"Removendo…":"Removing…"):(pt?"Desconectar":"Disconnect")}</button></div>
        </section>
        <section className="connection-card"><h3>{pt?"Atividade e notificações":"Activity and notifications"}</h3><p className="connection-section-hint">{pt?"Escolha o que aparece para seus contatos e como receber avisos.":"Choose what contacts see and how you receive alerts."}</p>
        <label className="preference-switch connection-switch"><input type="checkbox" checked={activity.sharing} onChange={event=>setMatrixActivityPreferences({sharing:event.target.checked})}/><span className="preference-switch-track"/><span>{pt?"Compartilhar atividade de jogo":"Share game activity"}<small>{pt?"Mostra o jogo em execução quando o servidor oferece presença.":"Shows your running game when the server supports presence."}</small></span></label>
        <label className="preference-switch connection-switch"><input type="checkbox" checked={activity.covers} onChange={event=>setMatrixActivityPreferences({covers:event.target.checked})}/><span className="preference-switch-track"/><span>{pt?"Mostrar capas nas atividades":"Show covers in activities"}<small>{pt?"Exibe uma capa pequena no chat do PeliGames.":"Displays a small cover in PeliGames chat."}</small></span></label>
        {activity.error&&<p role="status">{activity.error}</p>}
        <MatrixNotificationSoundPreferences/>
        </section>
      </div></section>
    </div>
    {!searchMode&&<div className="connection-privacy-note"><MenuIcon name="info"/><p>{pt?"As credenciais ficam no chaveiro do sistema. Apagar a pasta de configurações não desconecta suas contas; use Desconectar para removê-las deste dispositivo.":"Credentials stay in the system keyring. Deleting the settings folder does not disconnect accounts; use Disconnect to remove them from this device."}</p></div>}
  </div>;
}
