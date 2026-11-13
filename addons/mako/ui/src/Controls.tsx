import type {AddonHost} from '../../../_infra/ui-sdk';
import type {Connection,Settings,Message} from './api';
export function Controls({host,api,settings,blocked,running,patch,gpus}:{host:AddonHost;api:Connection;settings:Settings;blocked:boolean;running:boolean;patch:(p:Partial<Settings>,live?:boolean)=>void;gpus:{model:string;pciAddress:string}[]}){
 const t=api.t;
 const toggle=(key:keyof Settings,label:Message,hint:Message,live=false)=><div className="mako-field"><span>{t(label)}<small>{t(hint)}</small></span><host.ui.Switch label={t(label)} enabled={Boolean(settings[key])} disabled={blocked||(running&&!live)} onToggle={()=>patch({[key]:!settings[key]},live)}/></div>;
 const number=(key:keyof Settings,label:Message,hint:Message,min:number,max:number,step=1,live=false)=><label className="mako-field mako-field--stacked"><span>{t(label)}<small>{t(hint)}</small></span><input key={String(settings[key])} type="number" min={min} max={max} step={step} defaultValue={Number(settings[key])} disabled={blocked||(running&&!live)} onBlur={event=>{const n=Number(event.target.value);if(!event.target.value||!Number.isFinite(n)||n<min||n>max){event.target.value=String(settings[key]);return;}if(n!==settings[key])patch({[key]:n},live)}} onKeyDown={event=>{if(event.key==='Enter')event.currentTarget.blur()}}/></label>;
 return <>
  <div className="mako-controls">
   {toggle('live_enabled','live_enabled','live_hint',true)}{toggle('adaptive','adaptive','adaptive_hint',true)}
   {number('multiplier','multiplier','multiplier_hint',2,5,1,true)}{number('target_fps','target','target_hint',10,1000,1,true)}
   {number('flow_scale','flow','flow_hint',.25,1,.05,true)}{toggle('performance','performance','performance_hint',true)}
   {number('base_fps_cap','cap','cap_hint',0,1000,1,true)}{number('refresh_threshold','threshold','threshold_hint',0,1000,1,true)}
   {toggle('ultra_performance','ultra','startup')}{toggle('allow_fp16','fp16','startup')}
   <label className="mako-field mako-field--stacked"><span>{t('gpu')}<small>{t('startup')}</small></span><host.ui.Choice appearance="compact" label={t('gpu')} value={settings.gpu} disabled={blocked||running} options={[{value:'',label:t('gpu_default')},...gpus.map(g=>({value:g.pciAddress,label:g.model})),...(settings.gpu&&!gpus.some(g=>g.pciAddress===settings.gpu)?[{value:settings.gpu,label:settings.gpu}]:[])]} onChange={gpu=>patch({gpu})}/></label>
  </div>
  <h4>{t('scaling')}</h4><div className="mako-controls">
   {toggle('scaling_enabled','scaling','startup')}
   <label className="mako-field mako-field--stacked"><span>{t('scaling_method')}</span><host.ui.Choice appearance="compact" label={t('scaling_method')} value={settings.scaling_method} disabled={blocked||running||!settings.scaling_enabled} options={['native','mako','ls1','ls1-performance'].map(value=>({value,label:value==='native'?t('native'):value}))} onChange={scaling_method=>patch({scaling_method})}/></label>
   {number('scaling_factor','factor','startup',1,2,.05)}{number('scaling_sharpness','sharpness','startup',0,1,.05)}
  </div>
 </>;
}
