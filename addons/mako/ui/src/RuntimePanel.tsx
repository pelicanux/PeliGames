import {useEffect,useRef,useState} from 'react';
import type {AddonHost} from '../../../_infra/ui-sdk';
import type {TransferProgress} from '../../../_infra/ui-sdk/helpers/downloadProgress';
import {createDownloadRateTracker} from '../../../_infra/ui-sdk/helpers/downloadProgress';
import type {Connection,State} from './api';
export function useRuntime(host:AddonHost,api:Connection){
 const [state,setState]=useState<State>(),[busy,setBusy]=useState(false),[error,setError]=useState(''),[progress,setProgress]=useState<TransferProgress>();
 const lock=useRef(false),[available,setAvailable]=useState<string>();
 const check=()=>run(async()=>{const update=await api.check() as {version?:string;compatible?:boolean};if(update?.compatible)setAvailable(update.version)});
 const run=(work:()=>Promise<unknown>)=>{if(lock.current)return;lock.current=true;setBusy(true);host.setBusy(true);setError('');
  void work().then(()=>api.state()).then(value=>{if(!host.lifecycle.signal.aborted)setState(value)}).catch(error=>{if(!host.lifecycle.signal.aborted)setError(String(error))}).finally(()=>{lock.current=false;if(!host.lifecycle.signal.aborted){setBusy(false);host.setBusy(false);setProgress(undefined)}});
 };
 useEffect(()=>{run(api.state)},[]);
 const download=()=>{const rate=createDownloadRateTracker();run(()=>api.download(available||state?.settings.runtime_version||'4.5.0',p=>{if(!host.lifecycle.signal.aborted)setProgress(rate(p))}))};
 return {state,busy,error,progress,run,download,check,available};
}
export function RuntimePanel({host,api,operation,blocked}:{host:AddonHost;api:Connection;operation:ReturnType<typeof useRuntime>;blocked:boolean}){
 const t=api.t,[confirm,setConfirm]=useState<string>();const {state,run,download}=operation;
 return <>
  <header className="mako-runtime-heading"><div><h3>{t('component')}</h3><p>{t('shared')}</p></div><span className="mod-status-badge">{state?.installed?state.settings.runtime_version:t('missing')}</span></header>
  <div className="mako-actions"><button type="button" className="mod-action-primary" disabled={blocked||!state} onClick={download}><host.ui.Icon name="download"/>{t(state?.installed?'update':'download')}</button><host.ui.Button disabled={blocked} onClick={operation.check}><host.ui.Icon name="repair"/>{t('check')}</host.ui.Button></div>
  {operation.available&&<p>{t('available')}: {operation.available}</p>}
  {state&&<host.ui.Path compact label={t('folder')} path={state.directory}/>}
  <p>{t('dll_notice')}</p>
  {state?.versions.map(version=><div className="mako-version" key={version.version}><span>{version.version}{version.current?` · ${t('current')}`:''}{version.in_use?` · ${t('in_use')}`:''}</span>
   {!version.current&&(confirm===version.version?<><p>{t('remove_confirm')}</p><host.ui.Button disabled={blocked} onClick={()=>setConfirm(undefined)}>{t('cancel')}</host.ui.Button><host.ui.Button disabled={blocked} onClick={()=>{setConfirm(undefined);run(()=>api.removeBackup(version.version))}}>{t('confirm')}</host.ui.Button></>:<host.ui.Button disabled={blocked||version.in_use} onClick={()=>setConfirm(version.version)}>{t('remove_backup')}</host.ui.Button>)}
  </div>)}
  {Boolean(state?.versions.length)&&<div className="mako-version">{confirm==='all'?<><p>{t('remove_all_confirm')}</p><host.ui.Button disabled={blocked} onClick={()=>setConfirm(undefined)}>{t('cancel')}</host.ui.Button><host.ui.Button disabled={blocked} onClick={()=>{setConfirm(undefined);run(api.remove)}}>{t('confirm')}</host.ui.Button></>:<host.ui.Button disabled={blocked||state?.versions.some(version=>version.in_use)} onClick={()=>setConfirm('all')}>{t('remove_all')}</host.ui.Button>}</div>}
 </>;
}
