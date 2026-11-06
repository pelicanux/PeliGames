import {useState} from "react";
import {useI18n} from "../../i18n/I18nContext";
import {useDiscordPresence,saveDiscordPresence,type PresencePreferences} from "../../services/discordPresence";
import {ModOptionSwitch} from "../ModOptionRow";
export function DiscordPresencePreferences(){
  const {t: tx} = useI18n();

 const presence=useDiscordPresence();
 const [busy,setBusy]=useState(false),[error,setError]=useState("");
 const save=async(patch:Partial<PresencePreferences>)=>{setBusy(true);setError("");try{await saveDiscordPresence({...presence.preferences,...patch})}catch(e){setError(String(e))}finally{setBusy(false)}};
 return <section className="connection-card discord-presence-card" aria-label={tx("ui-discord-presence-preferences", "discord_activity")}>
  <h3>Discord Rich Presence</h3>
  <p className="connection-section-hint">{tx("ui-discord-presence-preferences", "shows_a_peligames_activity_only_while_recording_the_screen_or_running_instant_replay_doe")}</p>
  <div className="mod-setting-row"><div className="mod-row-copy"><strong>{tx("ui-discord-presence-preferences", "show_recording_and_replay_on_discord")}</strong><small>{tx("ui-discord-presence-preferences", "the_activity_disappears_when_capture_ends_video_audio_and_file_paths_are_not_sent")}</small></div><ModOptionSwitch label={tx("ui-discord-presence-preferences", "show_recording_and_replay_on_discord")} enabled={presence.preferences.enabled} disabled={busy||!presence.ready} onToggle={()=>void save({enabled:!presence.preferences.enabled})}/></div>
  <p role="status" className="discord-presence-status">{!presence.ready?(tx("ui-discord-presence-preferences", "loading_preferences")):!presence.preferences.enabled?(tx("ui-discord-presence-preferences", "activity_sharing_is_off")):!presence.active?(tx("ui-discord-presence-preferences", "waiting_for_recording_or_replay_no_activity_published")):presence.connected?(tx("ui-discord-presence-preferences", "recording_replay_shared_on_discord")):(tx("ui-discord-presence-preferences", "waiting_for_discord_desktop_reconnects_automatically"))}</p>
  {presence.preferences.enabled&&<p className="connection-section-hint">{tx("ui-discord-presence-preferences", "enable_activity_sharing_in_discord_s_privacy_settings_display_depends_on_your_discord_pr")}</p>}
  {(error||presence.error)&&<p role="alert" className="mod-feedback-error">{error||presence.error}</p>}
  {"__PELI_UI_PREVIEW__" in window&&<p className="connection-section-hint">{tx("ui-discord-presence-preferences", "preview_simulated_connection_no_activity_is_published_to_discord")}</p>}
 </section>;
}
