import {useEffect, useRef} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import type {GameInfo} from "../../components/GameGrid";
import {listPeliGamesEntries} from "../../services/installedLibrary";
export function useLauncherOpenRequest(initialGamePath: string | undefined, onOpen: (game: GameInfo) => void) {
  const callback = useRef(onOpen); callback.current = onOpen;
  useEffect(() => {
    let active = true, revision = 0;
    let unlisten: (() => void) | undefined;
    const showEntry = async (path: string) => {
      const request = ++revision;
      const entries = await listPeliGamesEntries(), entry = entries.find(game => game.path === path);
      if (active && request === revision && entry) callback.current(entry);
    };
    if (initialGamePath) void showEntry(initialGamePath).catch(console.error);
    if (!("__PELI_UI_PREVIEW__" in window)) {
      const receive = async () => {const path = await invoke<string | null>("take_launcher_open_request"); if (active && path) await showEntry(path);};
      void listen("launcher-open-request", () => {void receive().catch(console.error);}).then(stop => {
        if (!active) {stop(); return;}
        unlisten = stop; void receive().catch(console.error);
      }).catch(console.error);
    }
    return () => {active = false; unlisten?.();};
  }, [initialGamePath]);
}
