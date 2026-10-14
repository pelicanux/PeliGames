import { useEffect, useRef, useState } from "react";
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
  const activePath = execution.pendingPath || (execution.active ? execution.session?.path : undefined);
  const snapshot = useRef<{path:string; source: "main" | "dlss"; game?:GameInfo} | null>(null);
  const [resolved, setResolved] = useState<GameInfo | null>(null);
  const current = useRef({selected, source}); current.current = {selected,source};
  useEffect(() => {
    if (!activePath) { snapshot.current = null; setResolved(null); return; }
    if (snapshot.current?.path === activePath) return;
    const {selected,source} = current.current;
    const game = selected?.path === activePath ? selected : [...(readGameLibrary("own") || []), ...(readGameLibrary("all") || [])].find(g=>g.path===activePath);
    snapshot.current = {path:activePath,source,game}; setResolved(game || null);
    let active=true;
    if (!game) void listPeliGamesEntries().then(entries => { const found=applyCustomCovers(entries).find(g=>g.path===activePath); if (active && found) setResolved(found); }).catch(()=>{});
    return () => {active=false;};
  }, [activePath]);
  const games:RunningGameSummary[] = nexusGames.filter(game=>game.running || game.id===nexusLaunching || game.id===nexusStopping).map(game=>({
    onDetails:()=>onDetails({...game.game, library_view:"nexus", nexus_id:game.id, nexus_platform:game.platform}, "nexus"),
    id:`nexus:${game.id}`, name:game.game.name, cover:game.game.cover_url, library:"Nexus Mods",
    state:game.id===nexusStopping ? "stopping" : game.running ? "running" : "starting",
    stopDisabled:game.id===nexusStopping || (nexusBusy && game.id!==nexusLaunching), onStop:()=>stopNexus(game.id)
  }));
  if (activePath) {
    const game = resolved?.path === activePath ? resolved : selected?.path===activePath ? selected : snapshot.current?.game;
    const origin = snapshot.current?.path===activePath ? snapshot.current.source : source;
    games.push({id:`peligames:${execution.session?.id || activePath}`, name:game?.name || activePath.split(/[\\/]/).filter(Boolean).pop() || "PeliGames", cover:game?.cover_url,
      library:origin==="dlss" ? "DLSSNR-AMD" : language==="pt" ? "Biblioteca principal" : "Main library",
      state:execution.cancellingPath===activePath || execution.session?.state==="stopping" ? "stopping" : execution.pendingPath ? "starting" : execution.session?.state==="running" ? "running" : "starting",
      onDetails:game ? ()=>onDetails(game, origin) : undefined, onStop:()=>execution.cancel()});
  }
  return games;
}
