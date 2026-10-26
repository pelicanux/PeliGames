import {useEffect,useSyncExternalStore} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import type {RunningGameSummary} from "../components/RunningGamesIndicator";
export interface PresencePreferences {enabled:boolean;share_game:boolean;share_capture:boolean}
export interface PresenceStatus {preferences:PresencePreferences;connected:boolean;error:string|null;application_id:string}
let state:PresenceStatus&{ready:boolean}={preferences:{enabled:false,share_game:true,share_capture:true},connected:false,error:null,application_id:"",ready:false};
const subscribers=new Set<()=>void>();
const update=(value:PresenceStatus)=>{state={...value,ready:true};subscribers.forEach(fn=>fn());};
export function useDiscordPresence(){return useSyncExternalStore(fn=>{subscribers.add(fn);return()=>{subscribers.delete(fn)};},()=>state);}
export async function saveDiscordPresence(preferences:PresencePreferences){update(await invoke<PresenceStatus>("discord_presence_preferences",{preferences}));}
/** Publish only actual running games; preparation and filenames stay private. */
export function useDiscordGamePresence(games:RunningGameSummary[]){
 const name=games.find(game=>game.state==="running")?.name||null;
 useEffect(()=>{let active=true;let unlisten:(()=>void)|undefined;
  const preview=(event:Event)=>{if(active)update((event as CustomEvent<PresenceStatus>).detail);};
  window.addEventListener("discord-presence-preview",preview);
  void(async()=>{try{unlisten=await listen<PresenceStatus>("discord-presence-changed",event=>{if(active)update(event.payload)});if(!active){unlisten();return;}const value=await invoke<PresenceStatus>("discord_presence_status");if(active)update(value);}catch(error){if(active)update({...state,error:String(error)});}})();
  return()=>{active=false;unlisten?.();window.removeEventListener("discord-presence-preview",preview);};
 },[]);
 useEffect(()=>{const timer=setTimeout(()=>{void invoke("discord_presence_game",{name}).catch(error=>update({...state,error:String(error)}));},250);return()=>clearTimeout(timer);},[name]);
}
