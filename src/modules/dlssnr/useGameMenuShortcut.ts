import {useMemo} from "react";
import {invoke} from "@tauri-apps/api/core";
import {appConfigStore} from "../appConfig/appConfig";
import {createGameShortcutWriter} from "./gameShortcuts";
export function useGameMenuShortcut() {
  return useMemo(() => createGameShortcutWriter(appConfigStore.update, change => invoke("update_shortcut_key_in_game", {gameDir: change.directory, shortcutKey: change.key, route: change.route})), []);
}
