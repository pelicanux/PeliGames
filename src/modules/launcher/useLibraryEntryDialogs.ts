import {useEffect, useRef, useState, type RefObject} from "react";
import type {GameInfo} from "../../components/GameGrid";
import {addLauncherGames, choosePeliGamesEntry, newLauncherGames, type LauncherGame} from "../../services/installedLibrary";
import {notifyError} from "../../services/notifications";
import {useI18n} from "../../i18n/I18nContext";
interface Mutation {busy: boolean; lock: RefObject<boolean>; setBusy: (busy: boolean) => void}
interface Props {
  mutation: Mutation;
  sessions: {id: string; path: string; state: string}[];
  onSelected: (game: GameInfo, changing: boolean) => void;
}
export function useLibraryEntryDialogs({mutation, sessions, onSelected}: Props) {
  const {t} = useI18n();
  const [choice, setChoice] = useState<{entries: GameInfo[]; prefix: string; changing?: boolean} | null>(null);
  const [choiceError, setChoiceError] = useState("");
  const [launcher, setLauncher] = useState<{path: string; games: LauncherGame[]} | null>(null);
  const [launcherError, setLauncherError] = useState("");
  const lastExecutions = useRef(new Map<string, string>());
  useEffect(() => {
    const active = new Map(sessions.filter(item => ["starting", "running", "stopping"].includes(item.state)).map(item => [item.id, item.path]));
    for (const [id, path] of lastExecutions.current) if (!active.has(id)) void newLauncherGames(path).then(games => {
      if (games.length) {setLauncherError(""); setLauncher({path, games});}
    }).catch(console.error);
    lastExecutions.current = active;
  }, [sessions]);
  useEffect(() => {if (choiceError) notifyError(t("ui-app", "game_executable"), choiceError);}, [choiceError]);
  useEffect(() => {if (launcherError) notifyError(t("ui-app", "launcher_games"), launcherError);}, [launcherError]);
  const offerExecutables = (entries: GameInfo[], prefix: string, changing = false) => {setChoiceError(""); setChoice({entries, prefix, changing});};
  const keepAll = () => {const first = choice?.entries[0], changing = Boolean(choice?.changing); setChoice(null); if (first && !changing) onSelected(first, false);};
  const confirm = async (path: string) => {
    if (!choice || mutation.lock.current) return;
    mutation.lock.current = true; mutation.setBusy(true); setChoiceError("");
    try {
      const game = await choosePeliGamesEntry(path);
      setChoice(null); onSelected(game, Boolean(choice.changing));
      window.dispatchEvent(new Event("peligamesLibraryChanged"));
    } catch (error) {setChoiceError(String(error));}
    finally {mutation.lock.current = false; mutation.setBusy(false);}
  };
  const addGames = async (added: LauncherGame[], declined: string[]) => {
    if (!launcher || mutation.lock.current) return;
    mutation.lock.current = true; mutation.setBusy(true); setLauncherError("");
    try {await addLauncherGames(launcher.path, added, declined); setLauncher(null); window.dispatchEvent(new Event("peligamesLibraryChanged"));}
    catch (error) {setLauncherError(String(error));}
    finally {mutation.lock.current = false; mutation.setBusy(false);}
  };
  return {choice, choiceError, launcher, launcherError, busy: mutation.busy, offerExecutables, keepAll, confirm, addGames, postpone: () => {if (!mutation.busy) setLauncher(null);}};
}
