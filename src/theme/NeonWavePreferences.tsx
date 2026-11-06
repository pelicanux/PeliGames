import {useI18n} from "../i18n/I18nContext";
import {setNeonWaveOptions,useNeonWaveOptions,useNeonWaveAudioState} from "./neonWavePreferences";
export function NeonWavePreferences(){
  const {t: tx} = useI18n();

 const options=useNeonWaveOptions(),state=useNeonWaveAudioState(),preview="__PELI_UI_PREVIEW__" in window;
 const labels={off: tx("ui-neon-wave-preferences", "audio_reaction_off"), connecting: tx("ui-neon-wave-preferences", "connecting_to_audio"), listening: tx("ui-neon-wave-preferences", "reacting_to_audio_output"), unavailable: tx("ui-neon-wave-preferences", "audio_unavailable_ambient_motion_active"), paused: tx("ui-neon-wave-preferences", "effect_paused_to_save_resources"), demo: tx("ui-neon-wave-preferences", "simulated_preview_beats")};
 return <section className="preference-panel neon-wave-preferences"><h2>Neon Wave</h2><p>{tx("ui-neon-wave-preferences", "soft_lava_synth_colors_and_audio_pulses_your_previous_palette_is_preserved_in_standard")}</p>
 <label className="neon-wave-option"><input type="checkbox" checked={options.audio} onChange={event=>setNeonWaveOptions({audio:event.target.checked})}/>{tx("ui-neon-wave-preferences", "react_to_computer_audio")}</label>
 <small>{tx("ui-neon-wave-preferences", "uses_the_default_audio_output_without_microphone_access_or_recordings_pauses_while_hidde")}</small>
 {preview&&<label className="neon-wave-option"><input type="checkbox" checked={options.demo} onChange={event=>setNeonWaveOptions({demo:event.target.checked})}/>{tx("ui-neon-wave-preferences", "simulate_preview_beats_no_real_audio_capture")}</label>}
 <label className="neon-wave-intensity" htmlFor="neon-wave-intensity">{tx("ui-neon-wave-preferences", "intensity")}<output>{options.intensity}%</output></label><input id="neon-wave-intensity" type="range" min="10" max="100" step="1" value={options.intensity} onChange={event=>setNeonWaveOptions({intensity:Number(event.target.value)})}/>
 <label className="neon-wave-intensity" htmlFor="neon-wave-reaction">{tx("ui-neon-wave-preferences", "music_response")}<output>{options.reaction}%</output></label><input id="neon-wave-reaction" type="range" min="0" max="100" step="1" value={options.reaction} onChange={event=>setNeonWaveOptions({reaction:Number(event.target.value)})}/>
 <small>{tx("ui-neon-wave-preferences", "controls_beat_strength_without_changing_background_brightness")}</small>
 <p role="status" className="neon-wave-audio-status">{labels[state.status]}</p>{state.error&&<small role="alert">{state.error}</small>}
 </section>;
}
