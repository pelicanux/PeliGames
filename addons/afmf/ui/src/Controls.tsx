import type {AddonHost} from "../../../_infra/ui-sdk";
import type {Connection,Settings,Message} from "./api";

export function Controls({host,api,settings,blocked,patch}:{host:AddonHost;api:Connection;settings:Settings;blocked:boolean;patch:(value:Partial<Settings>)=>void}){
 const t=api.t;
 const choices=(key:"performance_mode"|"search_mode"|"fast_motion",label:Message,hint:Message,options:Message[])=><label className="afmf-field afmf-field--stacked"><span>{t(label)}<small>{t(hint)}</small></span><host.ui.Choice appearance="compact" label={t(label)} value={settings[key]} disabled={blocked} options={options.map(value=>({value,label:t(value)}))} onChange={value=>patch({[key]:value})}/></label>;
 return <div className="afmf-controls">
  {choices("performance_mode","mode","mode_hint",["auto","quality","performance"])}
  {choices("search_mode","search","search_hint",["auto","standard","high"])}
  {choices("fast_motion","fast","fast_hint",["blend","repeat"])}
  <label className="afmf-field afmf-field--stacked"><span>{t("minimum")}<small>{t("minimum_hint")}</small></span><input type="number" min={0} max={240} value={settings.min_fps} disabled={blocked} onChange={event=>patch({min_fps:Math.max(0,Math.min(240,Math.round(Number(event.target.value)||0)))})}/></label>
  {([['hud_detect','hud','hud_hint'],['pacing','pacing','pacing_hint']] as const).map(([key,label,hint])=><div className="afmf-field" key={key}><span>{t(label)}<small>{t(hint)}</small></span><host.ui.Switch label={t(label)} enabled={settings[key]} disabled={blocked} onToggle={()=>patch({[key]:!settings[key]})}/></div>)}
 </div>;
}
