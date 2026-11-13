export interface ReplayLaunchGame {key:string;path:string;nexusId?:string}
export interface ReplayLaunchSnapshot {sessions:{id:string;path:string;state:string}[];nexus:{id:string;running:boolean}[]}
export interface AutomaticReplayOptions {game_key:string;replay_on_launch?:boolean;[key:string]:unknown}
interface CaptureState {phase:string;game_key:string|null;replay:boolean}
interface Dependencies {settings:(key:string)=>Promise<AutomaticReplayOptions|null>;status:()=>Promise<CaptureState>;start:(options:AutomaticReplayOptions)=>Promise<unknown>;stop:()=>Promise<unknown>;error:(error:unknown)=>void;now?:()=>number}
interface Job {game:ReplayLaunchGame;confirmed:boolean;attempted:boolean;started:boolean;cancelled:boolean;since:number;oldIds:Set<string>;seenRunning:boolean}
/** Only explicit Play requests can arm capture. Polling a pre-existing game does not start it. */
export function createReplayOnPlay(deps:Dependencies){
 let snapshot:ReplayLaunchSnapshot={sessions:[],nexus:[]};const jobs=new Map<string,Job>();const now=deps.now??Date.now;
 const state=(job:Job)=>job.game.nexusId?(snapshot.nexus.find(g=>g.id===job.game.nexusId)?.running?"running":job.seenRunning?"exited":"starting"):[...snapshot.sessions].reverse().find(s=>s.path===job.game.path&&!job.oldIds.has(s.id))?.state??(job.seenRunning?"exited":undefined);
 const stopOwned=async(job:Job)=>{if(!job.started)return;try{const current=await deps.status();if(current.game_key===job.game.key&&["starting","active"].includes(current.phase))await deps.stop();}catch{/* Native lifetime cleanup may have already stopped it. */}};
 const start=async(job:Job)=>{try{
  const options=await deps.settings(job.game.key);if(job.cancelled||!options?.replay_on_launch||state(job)!=="running")return;
  const current=await deps.status();if(job.cancelled||state(job)!=="running")return;
  if(current.phase!=="idle"){if(current.game_key!==job.game.key||!current.replay)deps.error("O replay automático não iniciou porque já existe outra captura ativa.");return;}
  job.started=true;await deps.start(options);if(job.cancelled)await stopOwned(job);
 }catch(error){job.started=false;if(!job.cancelled)deps.error(error);}};
 const update=(next:ReplayLaunchSnapshot)=>{snapshot=next;for(const [key,job]of jobs){if(!job.confirmed)continue;const phase=state(job);
  if(["exited","cancelled","failed","stopping"].includes(phase??"")||!job.attempted&&now()-job.since>180000){job.cancelled=true;jobs.delete(key);void stopOwned(job);continue;}
  if(phase==="running"){job.seenRunning=true;if(!job.attempted){job.attempted=true;void start(job);}}
 }};
 return {update,launch:async<T>(game:ReplayLaunchGame,request:()=>Promise<T>):Promise<T>=>{
  const job:Job={game,confirmed:false,attempted:false,started:false,cancelled:false,since:now(),oldIds:new Set(snapshot.sessions.map(s=>s.id)),seenRunning:false};
  const previous=jobs.get(game.key);if(previous)previous.cancelled=true;jobs.set(game.key,job);
  try{const result=await request();if(result===false){job.cancelled=true;jobs.delete(game.key);}else{job.confirmed=true;update(snapshot);}return result;}
  catch(error){job.cancelled=true;jobs.delete(game.key);throw error;}
 },dispose:()=>{for(const job of jobs.values())job.cancelled=true;jobs.clear();}};
}
