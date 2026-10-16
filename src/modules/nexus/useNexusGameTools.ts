import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { NexusGame } from "./useNexusWorkspace";
import type { GameExtras } from "./NexusGameChecks";

type Module = { domain: string; tools?: {name: string}[]; activation?: {kind: string} | null; saves?: unknown };
const emptyExtras = { checks: [], extender_errors: [], redirection: null } satisfies GameExtras;

/** Shared by the tab strip and its panels, so capabilities survive tab changes. */
export function useNexusGameTools(game?: NexusGame) {
  const key = game ? `${game.id}:${game.adapter}` : "";
  const revision = game?.mods.map(mod => `${mod.id}:${mod.installed}:${mod.enabled}`).join("|");
  const [module, setModule] = useState<{key: string; value?: Module}>({key: ""});
  const [checks, setChecks] = useState<{key: string; value: GameExtras}>({key: "", value: emptyExtras});
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    setError("");
    if (game?.adapter) void invoke<{definitions: Module[]}>("list_nexus_modules").then(registry => {
      if (active) setModule({key, value: registry.definitions.find(item => item.domain === game.adapter)});
    }).catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [key]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    let active = true;
    if (game?.adapter) void invoke<GameExtras>("nexus_game_checks", {gameId: game.id}).then(value => {
      if (active) setChecks({key, value});
    }).catch(() => { /* Some modules have no additional game checks. */ });
    return () => { active = false; };
  }, [key, revision, game?.running]); // eslint-disable-line react-hooks/exhaustive-deps
  const definition = key && module.key === key ? module.value : undefined;
  const extras = key && checks.key === key ? checks.value : null;
  const tools = definition?.tools ?? [];
  const loadOrderKind = definition?.activation?.kind ?? "";
  const savesSupported = Boolean(definition?.saves);
  const available = tools.length > 0 || savesSupported || ["bethesda", "baldurs_gate3", "archive_list"].includes(loadOrderKind)
    || Boolean(extras && (extras.checks.length || extras.extender_errors.length || extras.redirection !== null));
  return {tools, loadOrderKind, savesSupported, extras, available, error,
    setExtras: (value: GameExtras) => setChecks({key, value})};
}
export type NexusGameTools = ReturnType<typeof useNexusGameTools>;
