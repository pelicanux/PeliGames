import {useEffect,useRef,useState} from "react";
import {convertFileSrc,invoke} from "@tauri-apps/api/core";
export type CaptureClip={name:string;title?:string;path:string;bytes:number;modified:number};
type Details={duration:number|null;width:number|null;height:number|null;fps:number|null;codec:string|null;thumbnail:string|null;pending:boolean;warning:string|null};
const duration=(seconds:number|null|undefined)=>{if(seconds==null)return "";const n=Math.floor(seconds);return `${Math.floor(n/60)}:${String(n%60).padStart(2,"0")}`;};
const titleOf=(clip:CaptureClip)=>clip.title||clip.name.replace(/\.[^.]+$/,"");
const formatOf=(clip:CaptureClip)=>{const extension=clip.name.split(".").pop()?.toLowerCase();return extension==="webm"?"WebM":extension==="mkv"?"MKV":extension==="mp4"?"MP4":"";};
const mediaUrl=(path:string)=>/^(data:|blob:|https?:|\/)/.test(path)&&"__PELI_UI_PREVIEW__" in window?path:convertFileSrc(path);
function VideoCard({clip,gameName,details,paused,onVisible,onPlay,onRename}:{clip:CaptureClip;gameName:string;details?:Details;paused:boolean;onVisible:()=>void;onPlay:()=>void;onRename:(title:string)=>Promise<void>}){
 const ref=useRef<HTMLElement>(null);const [editing,setEditing]=useState(false),[draft,setDraft]=useState(titleOf(clip)),[busy,setBusy]=useState(false),[error,setError]=useState("");
 useEffect(()=>{const element=ref.current;if(!element)return;const observer=new IntersectionObserver(entries=>{if(entries.some(e=>e.isIntersecting)){onVisible();observer.disconnect();}},{rootMargin:"100px"});observer.observe(element);return()=>observer.disconnect();},[onVisible]);
 const save=async()=>{setBusy(true);setError("");try{await onRename(draft);setEditing(false);}catch(e){setError(String(e));}finally{setBusy(false);}};
 return <article ref={ref} className="capture-video-card">
  <button type="button" className="capture-video-thumbnail" aria-label={`Assistir ${titleOf(clip)}`} onClick={onPlay}>
   {details?.thumbnail?<img src={mediaUrl(details.thumbnail)} alt="" loading="lazy"/>:<span className="capture-video-placeholder" aria-hidden="true">▶</span>}
   <span className="capture-video-play" aria-hidden="true">▶</span>{details?.duration!=null&&<span className="capture-video-duration">{duration(details.duration)}</span>}
  </button>
  <div className="capture-video-body">
   {editing?<form onSubmit={e=>{e.preventDefault();void save();}} className="capture-video-title-form"><input autoFocus aria-label="Título do vídeo" maxLength={120} value={draft} disabled={busy} onChange={e=>setDraft(e.target.value)} onKeyDown={e=>{if(e.key==="Escape"){e.stopPropagation();setEditing(false);}}}/><div><button type="submit" className="modern-mini-btn" disabled={busy||paused||!draft.trim()}>Salvar</button><button type="button" className="modern-mini-btn" disabled={busy} onClick={()=>setEditing(false)}>Cancelar</button></div></form>:<div className="capture-video-title"><h4>{titleOf(clip)}</h4><button type="button" className="modern-mini-btn" aria-label={`Editar título de ${titleOf(clip)}`} disabled={paused} onClick={()=>{setDraft(titleOf(clip));setEditing(true);}}>Editar título</button></div>}
   <p className="capture-video-game">{gameName}</p>
   <p className="capture-video-facts">{details?.width&&details.height?`${details.width} × ${details.height}`:"Resolução indisponível"}{details?.fps?` · ${Math.round(details.fps*100)/100} FPS`:""}{formatOf(clip)?` · ${formatOf(clip)}`:""}{details?.codec?` · ${details.codec.toUpperCase()}`:""}</p>
   <p className="capture-video-facts">{new Date(clip.modified*1000).toLocaleString()} · {(clip.bytes/1024/1024).toFixed(1)} MB</p>
   {details?.pending&&<small>Miniatura pendente até a captura terminar.</small>}
   {details?.warning&&<small>{details.warning}</small>}{error&&<p role="alert" className="capture-error">{error}</p>}
  </div>
 </article>;
}
export function CaptureVideoLibrary({clips,gameKey,gameName,folder,paused,onOpen,onRefresh}:{clips:CaptureClip[];gameKey:string;gameName:string;folder:string;paused:boolean;onOpen:(path:string)=>void;onRefresh:()=>Promise<void>}){
 const [limit,setLimit]=useState(12),[details,setDetails]=useState<Record<string,Details>>({}),[visible,setVisible]=useState<string[]>([]),[error,setError]=useState(""),[refreshing,setRefreshing]=useState(false);
 const [generationTick,setGenerationTick]=useState(0),[opening,setOpening]=useState(false);
 const generation=useRef(false),mounted=useRef(true);
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 useEffect(()=>{if(!paused)setDetails(old=>Object.fromEntries(Object.entries(old).filter(([,value])=>!value.pending)));},[paused]);
 const cacheKey=(clip:CaptureClip)=>`${clip.path}:${clip.bytes}:${clip.modified}`;
 useEffect(()=>{
   if(paused||generation.current)return;
   const clip=clips.slice(0,limit).find(c=>visible.includes(c.path)&&!details[cacheKey(c)]);if(!clip)return;
   generation.current=true;
   void invoke<Details>("capture_clip_details",{gameKey,gameName,path:clip.path}).then(result=>{if(mounted.current)setDetails(old=>({...old,[cacheKey(clip)]:result}));}).catch(e=>{if(mounted.current)setDetails(old=>({...old,[cacheKey(clip)]:{duration:null,width:null,height:null,fps:null,codec:null,thumbnail:null,pending:false,warning:String(e)}}));}).finally(()=>{generation.current=false;if(mounted.current)setGenerationTick(n=>n+1);});
 },[clips,gameKey,gameName,visible,limit,details,paused,generationTick]);
 const play=async(clip:CaptureClip)=>{if(opening)return;setOpening(true);setError("");try{if("__PELI_UI_PREVIEW__" in window){onOpen(clip.path);return;}await invoke("capture_clip_open",{gameKey,gameName,path:clip.path});}catch(e){if(mounted.current)setError(String(e));}finally{if(mounted.current)setOpening(false);}};
 const rename=async(clip:CaptureClip,title:string)=>{const saved=await invoke<{title:string;name:string;path:string}>("capture_clip_title",{gameKey,gameName,path:clip.path,title});if(mounted.current){setDetails(old=>{const current=old[cacheKey(clip)];return current?{...old,[cacheKey({...clip,path:saved.path})]:current}:old;});await onRefresh();}};
 const refresh=async()=>{setRefreshing(true);setError("");try{await onRefresh();setDetails({});}catch(e){setError(String(e));}finally{if(mounted.current)setRefreshing(false);}};
 return <section className="modern-performance-card capture-video-library">
  <div className="capture-library-heading"><div><h3>Vídeos deste jogo</h3><p>{folder||"Gravações e clipes salvos"}</p></div><div className="capture-preview-actions"><button type="button" className="modern-mini-btn" disabled={refreshing} onClick={()=>void refresh()}>Atualizar vídeos</button><button type="button" className="modern-mini-btn" disabled={!clips.length} onClick={()=>onOpen(folder)}>Abrir pasta</button></div></div>
  {"__PELI_UI_PREVIEW__" in window&&<p>Vídeos de demonstração com imagem sintética. Nenhuma tela ou áudio foi capturado.</p>}
  {paused&&clips.length>0&&<p>As miniaturas novas serão preparadas quando a captura terminar.</p>}
  {clips.length?<div className="capture-video-grid">{clips.slice(0,limit).map(original=>{const clip=original;return <VideoCard key={clip.path} clip={clip} gameName={gameName} paused={paused} details={details[cacheKey(clip)]} onVisible={()=>setVisible(old=>old.includes(clip.path)?old:[...old,clip.path])} onPlay={()=>void play(clip)} onRename={title=>rename(clip,title)}/>;})}</div>:<p>Nenhum clipe salvo para este jogo.</p>}
  {limit<clips.length&&<button type="button" className="modern-mini-btn capture-video-more" onClick={()=>setLimit(n=>n+12)}>Mostrar mais vídeos</button>}
  {opening&&<p role="status">Abrindo vídeo…</p>}
  {error&&<p role="alert" className="capture-error">{error}</p>}

 </section>;
}
