import { useState } from "react";
import { useI18n } from "../../i18n/I18nContext";
import { playNotificationSound, setNotificationSoundPreferences, unlockNotificationSound, useNotificationSoundPreferences } from "../../services/notificationSound";

export function MatrixNotificationSoundPreferences() {
  const { language } = useI18n();
  const pt = language === "pt", preferences = useNotificationSoundPreferences();
  const [result, setResult] = useState("");
  return <div className="matrix-notification-sound-settings">
    <label className="preference-switch"><input type="checkbox" checked={preferences.enabled} onChange={event => { setNotificationSoundPreferences({ enabled: event.target.checked }); setResult(""); }}/><span className="preference-switch-track"/><span>{pt ? "Som para menções e respostas" : "Sound for mentions and replies"}<small>{pt ? "Um bip retrô curto acompanha os avisos do Matrix." : "A short retro beep accompanies Matrix alerts."}</small></span></label>
    <div className="matrix-notification-sound-controls">
      <label htmlFor="matrix-notification-volume">{pt ? "Volume" : "Volume"} · {preferences.volume}%</label>
      <input id="matrix-notification-volume" type="range" min="0" max="100" step="1" disabled={!preferences.enabled} value={preferences.volume} onChange={event => { setNotificationSoundPreferences({ volume: Number(event.target.value) }); setResult(""); }}/>
      <button type="button" className="btn btn-secondary" disabled={!preferences.enabled || preferences.volume === 0} onClick={async () => {
        await unlockNotificationSound();
        const played = await playNotificationSound(true);
        setResult(played ? pt ? "Som de teste reproduzido." : "Test sound played." : pt ? "O áudio ainda não foi liberado. Clique novamente em Testar som." : "Audio is not unlocked yet. Click Test sound again.");
      }}>{pt ? "Testar som" : "Test sound"}</button>
    </div>
    {result && <small role="status">{result}</small>}
  </div>;
}
