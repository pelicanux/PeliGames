import { useState } from "react";
import { useI18n } from "../../i18n/I18nContext";
import { playNotificationSound, setNotificationSoundPreferences, unlockNotificationSound, useNotificationSoundPreferences } from "../../services/notificationSound";

export function MatrixNotificationSoundPreferences() {
  const {t: tx} = useI18n();

  const preferences = useNotificationSoundPreferences();
  const [result, setResult] = useState("");
  return <div className="matrix-notification-sound-settings">
    <label className="preference-switch"><input type="checkbox" checked={preferences.enabled} onChange={event => { setNotificationSoundPreferences({ enabled: event.target.checked }); setResult(""); }}/><span className="preference-switch-track"/><span>{tx("ui-matrix-notification-sound-preferences", "sound_for_mentions_and_replies")}<small>{tx("ui-matrix-notification-sound-preferences", "a_short_retro_beep_accompanies_matrix_alerts")}</small></span></label>
    <div className="matrix-notification-sound-controls">
      <label htmlFor="matrix-notification-volume">{"Volume"} · {preferences.volume}%</label>
      <input id="matrix-notification-volume" type="range" min="0" max="100" step="1" disabled={!preferences.enabled} value={preferences.volume} onChange={event => { setNotificationSoundPreferences({ volume: Number(event.target.value) }); setResult(""); }}/>
      <button type="button" className="btn btn-secondary" disabled={!preferences.enabled || preferences.volume === 0} onClick={async () => {
        await unlockNotificationSound();
        const played = await playNotificationSound(true);
        setResult(played ? tx("ui-matrix-notification-sound-preferences", "test_sound_played") : tx("ui-matrix-notification-sound-preferences", "could_not_play_the_sound_audio_output_unavailable"));
      }}>{tx("ui-matrix-notification-sound-preferences", "test_sound")}</button>
    </div>
    {result && <small role="status">{result}</small>}
  </div>;
}
