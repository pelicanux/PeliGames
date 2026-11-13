import {useEffect,useRef,useState} from 'react';
import type {AddonHost,AddonInstance,AddonLaunchState,AddonViewProps} from '../../../_infra/ui-sdk';
import {createLatestWriteQueue} from '../../../_infra/ui-sdk/helpers/latestWriteQueue';
import {connect,type Settings,type State,type Sandbox} from './api';
import {RuntimePanel,useRuntime} from './RuntimePanel';
import {Controls} from './Controls';
import './mako.css';
const projects=[{name:'MAKO',url:'https://github.com/eugeniosegala/MAKO'}];
export function createAddon(host:AddonHost):AddonInstance{
 const api=connect(host),t=api.t;
 function Feedback({operation}:{operation:ReturnType<typeof useRuntime>}){return <>{operation.error&&<p className="mod-feedback-error" role="alert">{operation.error}</p>}{operation.progress&&<host.ui.Progress progress={operation.progress} label={t('download_progress')}/>}</>}
 function Preferences({disabled}:AddonViewProps){const operation=useRuntime(host,api);return <div className="mako-addon mod-workspace"><RuntimePanel host={host} api={api} operation={operation} blocked={disabled||operation.busy}/><Feedback operation={operation}/><host.ui.Projects projects={projects}/></div>}
 function Game({context,disabled}:AddonViewProps){
  const [category,setCategory]=useState('prepare'),[state,setState]=useState<State>(),[settings,setSettings]=useState<Settings>(),[launch,setLaunch]=useState<AddonLaunchState>(),[error,setError]=useState(''),[saving,setSaving]=useState(false),[saved,setSaved]=useState(false),[gpus,setGpus]=useState<{model:string;pciAddress:string}[]>([]),[sandbox,setSandbox]=useState<Sandbox>();
  const operation=useRuntime(host,api),current=useRef<{settings:Settings;profile:string|null}|undefined>(undefined);
  const game=context.game;
  const runningRef=useRef(false);
  const [queue]=useState(()=>createLatestWriteQueue<{settings:Settings;live:boolean},void>(async next=>{
   if(!game||!host.game?.launch||!current.current||!launchRef.current)throw new Error(t('bridge_missing'));
   const own=launchRef.current,previous=current.current;
   if(next.live&&runningRef.current){if(!previous.profile)throw new Error(t('prepare_first'));await api.live(previous.profile,next.settings);current.current={...previous,settings:next.settings};}
   else{const data=await api.prepare(own.gameKey||game.id,own.gameName||'MAKO',game.executableDirectory||own.directory||game.directory,next.settings,previous.profile,own.options);await host.game.launch.save(game.id,data);current.current={settings:data,profile:data.profile};}
  },(_next,_result,failure)=>{if(host.lifecycle.signal.aborted)return;setSaving(false);host.setBusy(false);setSettings(current.current?.settings);setSaved(!failure);setError(failure?String(failure):'')}));
  const launchRef=useRef(launch);launchRef.current=launch;
  useEffect(()=>()=>queue.dispose(),[queue]);
  useEffect(()=>{let active=true;if(!game||!host.game?.launch){setError(t('bridge_missing'));return;}
   void host.game.launch.read(game.id).then(async own=>{
    const data=own.settings as {profile?:string;legacy_profile?:string;enabled?:boolean}|null;
    const profiles=[data?.profile||data?.legacy_profile,...(own.wrappers||[]).filter(w=>w.enabled).map(w=>w.program)].filter((p):p is string=>Boolean(p));
    const value=await api.inspect(own.gameKey||game.id,own.gameName||'MAKO',profiles);
    const enabled=data?Boolean(data.enabled):Boolean(value.profile&&own.wrappers?.some(w=>w.enabled&&w.program===value.profile));
    let next={...value.settings,enabled,flatpak:own.flatpak};
    if(host.files.machine){try{const machine=await host.files.machine();if(active&&!host.lifecycle.signal.aborted)setGpus(machine.gpus);if(!next.dll)next={...next,dll:await api.detect(machine.steam_libraries)||''};}catch(e){if(active&&!host.lifecycle.signal.aborted)setError(String(e));}}
    if(active&&!host.lifecycle.signal.aborted){setState(value);setSettings(next);setLaunch(own);current.current={settings:next,profile:value.profile};}
   }).catch(e=>{if(active&&!host.lifecycle.signal.aborted)setError(String(e))});return()=>{active=false};
  },[game?.id]);
  const running=Boolean(game?.running||launch?.running);runningRef.current=running;const blocked=disabled||operation.busy||saving||!settings;
  const patch=(patch:Partial<Settings>,live=false)=>{if(blocked||!settings||(running&&!live))return;const next={...settings,...patch};setSettings(next);setSaving(true);host.setBusy(true);setError('');setSaved(false);queue.enqueue({settings:next,live})};
  const ready=Boolean(operation.state?.installed||launch?.flatpak),canEnable=ready&&launch?.supported&&!launch.options.some(o=>['hdr','mlfg','vkd3d_latency','vulkan_latency','obs_capture'].includes(o));
  const chooseDll=async()=>{if(blocked||running||!host.files.chooseFile)return;try{const dll=await host.files.chooseFile('Lossless.dll',['dll']);if(dll)patch({dll})}catch(e){setError(String(e))}};
  const versions=operation.state?.versions||state?.versions||[];
  return <div className="mako-addon mod-workspace"><host.ui.Categories label="MAKO" active={category} onChange={setCategory} categories={[
   {id:'management',label:t('manage'),icon:'download',content:<RuntimePanel host={host} api={api} operation={operation} blocked={blocked||running}/>},
   {id:'prepare',label:t('prepare'),icon:'settings',content:<><h3>{t('prepare')}</h3><p>{t('description')}</p>
    {settings?<>
     <div className="mako-field"><span>{t('enable')}<small>{t('next_launch')}</small></span><host.ui.Switch label={t('enable')} enabled={settings.enabled} disabled={blocked||running||(!settings.enabled&&!canEnable)} onToggle={()=>patch({enabled:!settings.enabled})}/></div>
     {!ready&&<p>{t('install_first')}</p>}{launch?.competing&&<p>{t('exclusive')}</p>}{!canEnable&&ready&&<p>{t('conflicts')}</p>}
     <div className="mako-field"><span>{t('dll')}<small>{t('dll_notice')}</small></span><host.ui.Button disabled={blocked||running||!host.files.chooseFile} onClick={()=>void chooseDll()}>{t('choose')}</host.ui.Button></div><host.ui.Path compact label={t('dll')} path={settings.dll}/>
     <label className="mako-field mako-field--stacked"><span>{t('version')}<small>{t('version_hint')}</small></span><host.ui.Choice appearance="compact" label={t('version')} value={settings.runtime_version} disabled={blocked||running} options={[...new Set([settings.runtime_version,...versions.map(v=>v.version)])].filter(Boolean).map(version=>({value:version,label:version}))} onChange={runtime_version=>patch({runtime_version})}/></label>
     {launch?.flatpak&&<><host.ui.Button disabled={blocked||running} onClick={()=>{if(!launch||!settings)return;operation.run(async()=>{setSandbox(await api.sandbox(launch.gameKey||game!.id,settings.dll,settings.runtime_version))})}}>{t('flatpak')}</host.ui.Button>{sandbox&&<><p>{sandbox.message}</p>{sandbox.commands?.map(command=><pre className="mako-command" key={command}>{command}</pre>)}</>}</>}
    </>:<p role="status">{t('loading')}</p>}</>},
   {id:'adjust',label:t('adjust'),icon:'sliders',content:<><h3>{t('adjust')}</h3>{settings&&<Controls host={host} api={api} settings={settings} blocked={blocked} running={running} patch={patch} gpus={gpus}/>}<p>{t('compatibility')}</p></>},
   {id:'how',label:t('how'),icon:'info',content:<><h3>{t('how')}</h3><p>{t('how_text')}</p>{game&&<host.ui.LaunchProfile inline gameId={game.id} disabled={blocked}/>}</>},
  ]}/>{running&&<p role="status">{t('running')}</p>}{saving&&<p role="status">{t('saving')}</p>}{saved&&!saving&&<p role="status">{t('saved')}</p>}{error&&<p role="alert" className="mod-feedback-error">{error}</p>}<Feedback operation={operation}/></div>;
 }
 function Overview({context}:AddonViewProps){
  const [settings,setSettings]=useState<Settings>();useEffect(()=>{let active=true;
   if(context.game&&host.game?.launch)void host.game.launch.read(context.game.id).then(async own=>{
    const data=own.settings as (Settings&{profile?:string;legacy_profile?:string})|null;
    if(!data?.enabled)return;
    const profile=data.profile||data.legacy_profile;
    const value=profile?await api.inspect(own.gameKey||context.game!.id,own.gameName||'MAKO',[profile]):undefined;
    if(active&&!host.lifecycle.signal.aborted)setSettings(value?.settings||data);
   }).catch(()=>{});return()=>{active=false}},[context.game?.id]);
  return settings?<div className="mako-overview"><strong>MAKO · {settings.adaptive?t('adaptive'):`${settings.multiplier}×`}</strong><span>{t('overview_hint')}</span></div>:null;
 }
 return {apiVersion:1,id:host.addon.id,version:host.addon.version,views:{game:Game,preferences:Preferences,overview:Overview},dispose(){}};
}
