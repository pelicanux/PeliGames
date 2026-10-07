import { useEffect, useState } from "react";
import { MenuIcon } from "./MenuIcon";
import type { GamescopeSettings } from "../services/advancedSettings";
export function GamescopeSettingsPanel({ value, onChange }: { value: GamescopeSettings; onChange: (value: GamescopeSettings) => void }) {
  const [extra, setExtra] = useState(value.additional_options);
  useEffect(() => setExtra(value.additional_options), [value.additional_options]);
  const update = (patch: Partial<GamescopeSettings>) => onChange({ ...value, ...patch });
  const toggle = (key: "enable_upscaling" | "enable_limiter" | "force_grab_cursor", title: string, help?: string) => <div className="advanced-option gamescope-toggle-row">
    <button type="button" role="switch" aria-label={title} aria-checked={value[key]} className={`advanced-toggle ${value[key] ? "enabled" : ""}`} onClick={() => update({ [key]: !value[key] })}><span /></button>
    <span>{title}</span>{help && <span className="gamescope-help" title={help}><MenuIcon name="info" /></span>}
  </div>;
  const number = (key: "width" | "height" | "output_width" | "output_height" | "fps" | "fps_unfocused", title: string) => <label key={key}>{title}<input type="number" aria-label={title} min="0" max={key.startsWith("fps") ? 1000 : 16384} placeholder="Padrão" defaultValue={value[key] || ""} onBlur={e => update({ [key]: Number(e.target.value) })} onKeyDown={e => { if (e.key === "Enter") e.currentTarget.blur(); }} /></label>;
  return <div className="gamescope-settings">
    {toggle("enable_upscaling", "Permitir aprimoramento de escala (Upscaling)")}
    {value.enable_upscaling && <div className="gamescope-fields">
      <label className="gamescope-full-row">Método de escala<select value={value.filter} onChange={e => update({ filter: e.target.value as GamescopeSettings["filter"] })}>
        <option value="fsr">AMD FidelityFX Super Resolution 1.0 (FSR)</option><option value="nis">NVIDIA Image Scaling (NIS)</option><option value="integer">Escala inteira (Integer Upscale)</option><option value="stretch">Esticar imagem</option>
      </select></label>
      {number("width", "Largura do jogo")}{number("height", "Altura do jogo")}
      {number("output_width", "Largura de saída")}{number("output_height", "Altura de saída")}
      <label className="gamescope-full-row">Tipo de janela<select value={value.window_type} onChange={e => update({ window_type: e.target.value as GamescopeSettings["window_type"] })}><option value="fullscreen">Tela cheia</option><option value="borderless">Sem borda</option><option value="windowed">Janela</option></select></label>
    </div>}
    {toggle("enable_limiter", "Habilitar limitador de FPS")}
    {value.enable_limiter && <div className="gamescope-fields">{number("fps", "Limite de FPS")}{number("fps_unfocused", "FPS sem foco")}</div>}
    {toggle("force_grab_cursor", "Habilitar captura forçada do cursor", "Mantém o mouse capturado dentro do jogo. Útil quando o cursor escapa ou o jogo perde o foco.")}
    <label className="gamescope-extra-label">Opções adicionais <span className="gamescope-help" title="Argumentos extras do Gamescope. Use aspas para valores com espaços. Não inclua comandos, %command% ou o separador --."><MenuIcon name="info" /></span>
      <input aria-label="Opções adicionais" value={extra} onChange={e => setExtra(e.target.value)} onBlur={() => { if (extra !== value.additional_options) update({ additional_options: extra.trim() }); }} onKeyDown={e => { if (e.key === "Enter") e.currentTarget.blur(); }} />
    </label>
  </div>;
}
