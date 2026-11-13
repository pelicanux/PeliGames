import {useEffect, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {GameInfo} from "../../components/GameGrid";

/** Game release metadata; optional integration status is provided by each add-on. */
export function useGameReleaseDate(selectedGame:GameInfo|null){
 const [releaseDate,setReleaseDate]=useState("Desconhecido");
 useEffect(()=>{let alive=true;setReleaseDate(selectedGame?.app_id?"Detectando...":"Desconhecido");
  if(selectedGame?.app_id)void invoke<string>("fetch_steam_release_date",{appId:selectedGame.app_id}).then(value=>{if(alive)setReleaseDate(value)}).catch(()=>{if(alive)setReleaseDate("Desconhecido")});
  return()=>{alive=false};
 },[selectedGame?.path,selectedGame?.app_id]);
 return releaseDate;
}
