import { SelectionMenu } from "./SelectionMenu";
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
      <div className="gamescope-full-row">Método de escala<SelectionMenu label="Método de escala" value={value.filter} onChange={filter => update({ filter: filter as GamescopeSettings["filter"] })} options={[
        {value:"fsr",label:"AMD FidelityFX Super Resolution 1.0 (FSR)"},{value:"nis",label:"NVIDIA Image Scaling (NIS)"},{value:"integer",label:"Escala inteira (Integer Upscale)"},{value:"stretch",label:"Esticar imagem"}
      ]} /></div>
      {number("width", "Largura do jogo")}{number("height", "Altura do jogo")}
      {number("output_width", "Largura de saída")}{number("output_height", "Altura de saída")}
      <div className="gamescope-full-row">Tipo de janela<SelectionMenu label="Tipo de janela" value={value.window_type} onChange={window_type => update({ window_type: window_type as GamescopeSettings["window_type"] })} options={[{value:"fullscreen",label:"Tela cheia"},{value:"borderless",label:"Sem borda"},{value:"windowed",label:"Janela"}]} /></div>
    </div>}
    {toggle("enable_limiter", "Habilitar limitador de FPS")}
    {value.enable_limiter && <div className="gamescope-fields">{number("fps", "Limite de FPS")}{number("fps_unfocused", "FPS sem foco")}</div>}
    {toggle("force_grab_cursor", "Habilitar captura forçada do cursor", "Mantém o mouse capturado dentro do jogo. Útil quando o cursor escapa ou o jogo perde o foco.")}
    <label className="gamescope-extra-label">Opções adicionais <span className="gamescope-help" title="Argumentos extras do Gamescope. Use aspas para valores com espaços. Não inclua comandos, %command% ou o separador --."><MenuIcon name="info" /></span>
      <input aria-label="Opções adicionais" value={extra} onChange={e => setExtra(e.target.value)} onBlur={() => { if (extra !== value.additional_options) update({ additional_options: extra.trim() }); }} onKeyDown={e => { if (e.key === "Enter") e.currentTarget.blur(); }} />
    </label>
  </div>;
}
