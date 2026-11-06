import { ConnectionStorageChoice } from "../ConnectionStorageChoice";
import { refreshMatrixStatus } from "../../services/matrixChat";
import { useI18n } from "../../i18n/I18nContext";
import { NexusAccountPanel } from "../../modules/nexus/NexusAccountPanel";
import { usePreferencesText } from "./labels";
import { DiscordConnection } from "./DiscordConnection";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { MenuIcon } from "../MenuIcon";
import { notifyError, notifyInfo } from "../../services/notifications";
import { MatrixChatPanel } from "../MatrixChatPanel";
import { MatrixIcon } from "../MatrixIcon";
import { MatrixNotificationSoundPreferences } from "./MatrixNotificationSoundPreferences";
import { displayName, disconnectMatrixChat, useMatrixChat } from "../../services/matrixChat";
import { setMatrixActivityPreferences, useMatrixActivityPreferences } from "../../services/matrixGameActivity";

const CONNECTION_TABS=[{id:"nexus",label:"Nexus Mods"},{id:"discord",label:"Discord"},{id:"matrix",label:"Matrix"}] as const;
type ConnectionTab=typeof CONNECTION_TABS[number]["id"];
export function ConnectionsPreferences({searchMode=false,focusTarget,searchControl}:{searchMode?:boolean;focusTarget?:string|null;searchControl?:ReactNode}) {
  const {t: tx} = useI18n();

  const text = usePreferencesText();

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
      const revoked = await disconnectMatrixChat();
      notifyInfo("Matrix", revoked
        ? (tx("ui-connections-preferences", "connection_removed_and_credentials_deleted_from_this_device"))
        : (tx("ui-connections-preferences", "credentials_removed_from_this_device_the_server_could_not_confirm_sign_out_also_end_this")));
    } catch (error) { notifyError("Matrix", String(error)); }
    finally { removalLock.current = false; setRemovingMatrix(false); }
  };
  return <div className="connections-workspace mod-workspace">
    {matrixOpen && <MatrixChatPanel onClose={() => setMatrixOpen(false)}/>}
    {!searchMode&&<><header className="mod-intro"><h3>{tx("ui-connections-preferences", "connect_the_services_you_use")}</h3><p>{tx("ui-connections-preferences", "link_an_account_check_access_and_choose_what_to_share")}</p></header>
    <div className="settings-tab-toolbar"><div className="preference-mod-tabs connection-tabs" role="tablist" aria-label={text.connections} data-active={tab} onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;event.preventDefault();const index=CONNECTION_TABS.findIndex(item=>item.id===tab);const next=CONNECTION_TABS[event.key==="Home"?0:event.key==="End"?2:(index+(event.key==="ArrowLeft"?-1:1)+3)%3].id;setTab(next);requestAnimationFrame(()=>document.getElementById(`connection-tab-${next}`)?.focus())}}>{CONNECTION_TABS.map(item=><button type="button" key={item.id} role="tab" id={`connection-tab-${item.id}`} aria-selected={tab===item.id} aria-controls={`connection-panel-${item.id}`} tabIndex={tab===item.id?0:-1} onClick={()=>setTab(item.id)}>{item.label}</button>)}</div>{searchControl}</div></>}
    <div className="connection-tab-panel" hidden={!searchMode&&tab!=="nexus"} role={searchMode?undefined:"tabpanel"} id="connection-panel-nexus" aria-labelledby={searchMode?undefined:"connection-tab-nexus"}><section className="connection-module" data-preference-id="connection-nexus"><NexusAccountPanel/></section></div>
    <div className="connection-tab-panel" hidden={!searchMode&&tab!=="discord"} role={searchMode?undefined:"tabpanel"} id="connection-panel-discord" aria-labelledby={searchMode?undefined:"connection-tab-discord"}><DiscordConnection/></div>
    <div className="connection-tab-panel" hidden={!searchMode&&tab!=="matrix"} role={searchMode?undefined:"tabpanel"} id="connection-panel-matrix" aria-labelledby={searchMode?undefined:"connection-tab-matrix"}>
      <section className="connection-module" data-preference-id="connection-matrix"><div className="connection-grid">
        <section className="connection-card"><header className="connection-heading"><span className="mod-component-icon"><MatrixIcon/></span><div><h3>Matrix</h3><p>{tx("ui-connections-preferences", "chat_with_the_community_in_the_launcher")}</p></div><span className="mod-status-badge" data-ready={Boolean(matrix.userId)} role="status">{matrix.userId?(tx("ui-connections-preferences", "connected")):matrix.pending?(tx("ui-connections-preferences", "waiting_for_sign_in")):(tx("ui-connections-preferences", "not_connected"))}</span></header>
        <ConnectionStorageChoice service="matrix" disabled={Boolean(matrix.userId)||matrix.pending||removingMatrix} onChange={()=>void refreshMatrixStatus()}/>
        <div className="connection-account-row"><MenuIcon name={matrix.userId?"check":"chainLinked"}/><div><strong>{matrix.userId?displayName(matrix.userId,matrix.names):(tx("ui-connections-preferences", "sign_in_to_your_account"))}</strong><p>{matrix.userId?matrix.userId:(tx("ui-connections-preferences", "open_the_chat_to_authorize_the_connection_in_your_browser"))}</p></div></div>
        <div className="connection-primary-actions"><button type="button" className="mod-action-primary" disabled={removingMatrix} onClick={()=>setMatrixOpen(true)}><MatrixIcon/>{matrix.userId?(tx("ui-connections-preferences", "open_matrix_chat")):(tx("ui-connections-preferences", "connect_matrix"))}</button></div>
        {matrix.userId && <div className="mod-setting-row mod-destructive-row"><div className="mod-row-copy"><strong>{tx("ui-connections-preferences", "remove_connection")}</strong><small>{tx("ui-connections-preferences", "deletes_credentials_from_this_device_your_account_is_preserved")}</small></div><button type="button" className="mod-action-quiet danger" disabled={removingMatrix||matrix.pending} aria-busy={removingMatrix} onClick={()=>void removeMatrix()}><MenuIcon name="chainBroken"/>{removingMatrix?(tx("ui-connections-preferences", "removing")):(tx("ui-connections-preferences", "disconnect"))}</button></div>}
        </section>
        <section className="connection-card"><h3>{tx("ui-connections-preferences", "activity_and_notifications")}</h3><p className="connection-section-hint">{tx("ui-connections-preferences", "choose_what_contacts_see_and_how_you_receive_alerts")}</p>
        <label className="preference-switch connection-switch"><input type="checkbox" checked={activity.sharing} onChange={event=>setMatrixActivityPreferences({sharing:event.target.checked})}/><span className="preference-switch-track"/><span>{tx("ui-connections-preferences", "share_game_activity")}<small>{tx("ui-connections-preferences", "shows_your_running_game_when_the_server_supports_presence")}</small></span></label>
        <label className="preference-switch connection-switch"><input type="checkbox" checked={activity.covers} onChange={event=>setMatrixActivityPreferences({covers:event.target.checked})}/><span className="preference-switch-track"/><span>{tx("ui-connections-preferences", "show_covers_in_activities")}<small>{tx("ui-connections-preferences", "displays_a_small_cover_in_peligames_chat")}</small></span></label>
        {activity.error&&<p role="status">{activity.error}</p>}
        <MatrixNotificationSoundPreferences/>
        </section>
      </div></section>
    </div>
    {!searchMode&&<div className="connection-privacy-note"><MenuIcon name="info"/><p>{tx("ui-connections-preferences", "saved_sessions_use_the_system_keyring_temporary_sessions_stay_in_memory_until_peligames_")}</p></div>}
  </div>;
}
