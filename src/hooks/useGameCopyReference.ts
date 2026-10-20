import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { GameInfo } from "../components/GameGrid";
export interface GameCopyReference {kind:string|null;base:string|null;reserved:boolean;dependents:string[]}
export function useGameCopyReference(game:GameInfo|null){
 const [state,setState]=useState<{path:string;reference:GameCopyReference|null}>({path:"",reference:null});
 useEffect(()=>{
  let alive=true;
  const refresh=()=>{
   if(!game||game.launcher!=="PeliGames"||"__PELI_UI_PREVIEW__" in window){setState({path:game?.path||"",reference:null});return;}
   void invoke<GameCopyReference>("get_game_copy_reference",{path:game.path}).then(reference=>{if(alive)setState({path:game.path,reference});}).catch(()=>{if(alive)setState({path:game.path,reference:null});});
  };
  refresh();window.addEventListener("peligamesLibraryChanged",refresh);window.addEventListener("focus",refresh);
  return()=>{alive=false;window.removeEventListener("peligamesLibraryChanged",refresh);window.removeEventListener("focus",refresh);};
 },[game?.path]);
 return state.path===game?.path?state.reference:null;
}
