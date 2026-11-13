import {useEffect,useState} from 'react';
import type {AddonHost} from '../../../_infra/ui-sdk';
import type {Api,Clip,Options} from './api';
export function createVideo(host:AddonHost,api:Api){
 const t=api.t;
 return function Video({clip,options,refresh,run,disabled}:{clip:Clip;options:Options;refresh:()=>Promise<void>;run:(work:()=>Promise<unknown>)=>Promise<void>;disabled:boolean}){
  const [editing,setEditing]=useState(false),[title,setTitle]=useState(clip.title),[details,setDetails]=useState<{duration?:number;width?:number;height?:number;fps?:number;codec?:string;warning?:string;preview?:string}>();
  useEffect(()=>{let live=true;void api.call<typeof details>('clips.details',{gameKey:options.game_key,gameName:options.game_name,path:clip.path}).then(v=>{if(live)setDetails(v)}).catch(()=>{});return()=>{live=false}},[clip.path]);
  return <article className="gsr-clip">{details?.preview&&<img className="gsr-thumbnail" src={details.preview} alt=""/>}<div><strong>{clip.title}</strong><small>{(clip.bytes/1048576).toFixed(1)} MB{details?.width?` · ${details.width}×${details.height}`:''}{details?.duration?` · ${Math.round(details.duration)}s`:''}{details?.codec?` · ${details.codec}`:''}</small>{details?.warning&&<small>{details.warning}</small>}</div><div className="gsr-actions">{editing?<><label className="gsr-field"><span>{t('title')}</span><input value={title} onChange={e=>setTitle(e.target.value)} maxLength={100}/></label><host.ui.Button disabled={disabled} onClick={()=>void run(async()=>{await api.call('clips.rename',{gameKey:options.game_key,gameName:options.game_name,path:clip.path,title});setEditing(false);await refresh()})}>{t('confirm')}</host.ui.Button><host.ui.Button disabled={disabled} onClick={()=>setEditing(false)}>{t('cancel')}</host.ui.Button></>:<><host.ui.Button disabled={disabled||!host.files.openFile} onClick={()=>void run(async()=>{const file=await api.call<string>('clips.validate',{gameKey:options.game_key,gameName:options.game_name,path:clip.path});await host.files.openFile?.(file)})}>{t('play')}</host.ui.Button><host.ui.Button disabled={disabled} onClick={()=>setEditing(true)}>{t('rename')}</host.ui.Button></>}</div></article>;
 }
}
