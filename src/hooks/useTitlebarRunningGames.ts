import { useEffect, useState } from "react";
import type { GameInfo } from "../components/GameGrid";
import type { RunningGameSummary } from "../components/RunningGamesIndicator";
import type { useGameExecution } from "./useGameExecution";
import type { NexusGame } from "../modules/nexus/useNexusWorkspace";
import { readGameLibrary, applyCustomCovers } from "../services/gameLibraryCache";
import { listPeliGamesEntries } from "../services/installedLibrary";
import { useI18n } from "../i18n/I18nContext";

export function useTitlebarRunningGames({ execution, selected, source, nexusGames, nexusLaunching, nexusStopping, nexusBusy, stopNexus, onDetails }: {
  execution: ReturnType<typeof useGameExecution>; selected: GameInfo | null; source: "main" | "dlss";
  nexusGames: NexusGame[]; nexusLaunching: string | null; nexusStopping: string | null; nexusBusy: boolean;
  stopNexus: (id: string) => Promise<unknown>;
  onDetails: (game: GameInfo, source: "main" | "dlss" | "nexus") => void;
}): RunningGameSummary[] {
  const { language } = useI18n();
  const activeSessions=execution.sessions.filter(item=>["starting","running","stopping"].includes(item.state));
  const paths=activeSessions.map(item=>item.path);
  if(execution.pendingPath&&!paths.includes(execution.pendingPath))paths.push(execution.pendingPath);
  const pathsKey=paths.join("\n");
  const [resolved,setResolved]=useState<GameInfo[]>([]);
  useEffect(()=>{
    let alive=true;
    const cached=[...(readGameLibrary("own")||[]),...(readGameLibrary("all")||[])];
    setResolved(cached);
    if(paths.length)void listPeliGamesEntries().then(entries=>{if(alive)setResolved([...applyCustomCovers(entries),...cached]);}).catch(()=>{});
    return()=>{alive=false;};
  },[pathsKey]);
  const games:RunningGameSummary[] = nexusGames.filter(game=>game.running || game.id===nexusLaunching || game.id===nexusStopping).map(game=>({
    onDetails:()=>onDetails({...game.game, library_view:"nexus", nexus_id:game.id, nexus_platform:game.platform}, "nexus"),
    id:`nexus:${game.id}`, name:game.game.name, cover:game.game.cover_url, library:"Nexus Mods",
    state:game.id===nexusStopping ? "stopping" : game.running ? "running" : "starting",
    stopDisabled:game.id===nexusStopping || (nexusBusy && game.id!==nexusLaunching), onStop:()=>stopNexus(game.id)
  }));
  for(const path of paths){
    const status=activeSessions.find(item=>item.path===path);
    const game=selected?.path===path?selected:resolved.find(item=>item.path===path);
    const origin=selected?.path===path?source:"main";
    games.push({id:`peligames:${status?.id||path}`,name:game?.name||path.split(/[\\/]/).filter(Boolean).pop()||"PeliGames",cover:game?.cover_url,
      library:origin==="dlss"?"DLSSNR-AMD":language==="pt"?"Biblioteca principal":"Main library",
      state:execution.cancellingPath===path||status?.state==="stopping"?"stopping":status?.state==="running"?"running":"starting",
      stopDisabled:status?.state==="stopping",onDetails:game?()=>onDetails(game,origin):undefined,onStop:()=>execution.cancel(path)});
  }
  return games;
}
