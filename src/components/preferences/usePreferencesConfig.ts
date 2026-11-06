import {appConfigStore} from "../../modules/appConfig/appConfig";
import {useAppConfig} from "../../modules/appConfig/useAppConfig";
import type {AppConfig} from "../../types/appConfig";

/** Preferences edit the shared draft; the rest of the app consumes confirmed settings. */
export function usePreferencesConfig() {
  const state = useAppConfig();
  return {
    config: state.draft, ready: state.ready, saving: state.pending > 0, error: state.error,
    save: (patch: Partial<AppConfig>) => {void appConfigStore.update(patch).catch(() => {});},
    retry: () => {void appConfigStore.retry().catch(() => {});},
    isPending: () => appConfigStore.getSnapshot().pending > 0,
    refresh: appConfigStore.refresh,
  };
}
