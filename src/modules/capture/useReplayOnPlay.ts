import {useEffect,useRef} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {GameInfo} from "../../components/GameGrid";
import {notifyError} from "../../services/notifications";
import {createReplayOnPlay,type ReplayLaunchSnapshot} from "./replayOnPlay";
export function useReplayOnPlay(sessions:ReplayLaunchSnapshot["sessions"],nexus:ReplayLaunchSnapshot["nexus"]){
 const controller=useRef<ReturnType<typeof createReplayOnPlay>|null>(null);
 if(!controller.current)controller.current=createReplayOnPlay({settings:gameKey=>invoke("capture_settings",{gameKey}),status:()=>invoke("capture_status"),start:options=>invoke("capture_start",{options,replay:true}),stop:()=>invoke("capture_action",{action:"stop"}),error:error=>notifyError("Replay automático",String(error))});
 const current=controller.current;
 useEffect(()=>{current.update({sessions,nexus});},[current,sessions,nexus]);
 useEffect(()=>()=>current.dispose(),[current]);
 return {launch:<T,>(game:GameInfo,request:()=>Promise<T>)=>current.launch({key:game.external_id||game.path,path:game.path,nexusId:game.nexus_id},request)};
}
