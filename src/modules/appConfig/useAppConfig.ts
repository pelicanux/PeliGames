import {useEffect, useSyncExternalStore} from "react";
import {appConfigStore} from "./appConfig";

export function useAppConfig() {
  const state = useSyncExternalStore(appConfigStore.subscribe, appConfigStore.getSnapshot, appConfigStore.getSnapshot);
  useEffect(() => {void appConfigStore.load().catch(() => {});}, []);
  return state;
}
