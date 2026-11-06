import {ExecutableChoiceModal} from "../../components/ExecutableChoiceModal";
import {LauncherGamesModal} from "../../components/LauncherGamesModal";
import type {useLibraryEntryDialogs} from "./useLibraryEntryDialogs";
export function LibraryEntryDialogs({controller, currentPath}: {controller: ReturnType<typeof useLibraryEntryDialogs>; currentPath?: string}) {
  const {choice, choiceError, launcher, launcherError, busy, keepAll, confirm, addGames, postpone} = controller;
  return <>
    {choice && <ExecutableChoiceModal entries={choice.entries} prefix={choice.prefix} changing={choice.changing} currentPath={currentPath} busy={busy} error={choiceError}
      onKeepAll={keepAll} onConfirm={path => void confirm(path)}/>}
    {launcher && <LauncherGamesModal games={launcher.games} busy={busy} error={launcherError} onLater={postpone} onAdd={(added, declined) => void addGames(added, declined)}/>}
  </>;
}
