import type {AppConfig} from "../../types/appConfig";

/** Editor defaults do not count as a configured installation until saved. */
export const editorDefaults: AppConfig = {backend: "AMDNR", dll_version: "0.5.1", shortcut_key: "Insert"};
export interface ConfigSnapshot {
  config: AppConfig | null;
  draft: AppConfig;
  ready: boolean;
  loaded: boolean;
  loading: boolean;
  pending: number;
  error: string;
}
interface Transport {
  load: () => Promise<AppConfig | null>;
  save: (config: AppConfig) => Promise<unknown>;
  committed?: (previous: AppConfig | null, next: AppConfig | null) => void;
}
interface Edit {id: number; patch: Partial<AppConfig>}
function merge(base: AppConfig, patch: Partial<AppConfig>): AppConfig {
  return {...base, ...patch,
    ...(patch.game_shortcut_keys && {game_shortcut_keys: {...base.game_shortcut_keys, ...patch.game_shortcut_keys}}),
    ...(patch.custom_game_paths && {custom_game_paths: {...base.custom_game_paths, ...patch.custom_game_paths}}),
  };
}

/** One ordered writer per launcher window; React owns neither persistence nor the queue. */
export function createAppConfigStore(transport: Transport) {
  let state: ConfigSnapshot = {config: null, draft: {...editorDefaults}, ready: false, loaded: false, loading: false, pending: 0, error: ""};
  let edits: Edit[] = [], sequence = 0;
  let tail: Promise<unknown> = Promise.resolve();
  let initialLoad: Promise<AppConfig | null> | undefined;
  const listeners = new Set<() => void>();
  function publish(patch: Partial<ConfigSnapshot>) {
    state = {...state, ...patch};
    state.draft = edits.reduce((config, edit) => merge(config, edit.patch), state.config ?? editorDefaults);
    listeners.forEach(listener => listener());
  }
  function schedule<T>(action: () => Promise<T>): Promise<T> {
    const result = tail.then(action);
    tail = result.catch(() => {});
    return result;
  }
  function accept(config: AppConfig | null) {
    const previous = state.config;
    publish({config, ready: true, loaded: true, error: edits.length ? state.error : ""});
    transport.committed?.(previous, config);
  }
  function refresh(): Promise<AppConfig | null> {
    publish({loading: true});
    return schedule(async () => {
      try {
        const config = await transport.load();
        accept(config);
        return config;
      } catch (error) {
        publish({loaded: true, error: String(error)});
        throw error;
      } finally {publish({loading: false});}
    });
  }
  function load(): Promise<AppConfig | null> {
    if (state.ready) return Promise.resolve(state.config);
    return initialLoad ??= refresh().finally(() => {initialLoad = undefined;});
  }
  function update(patch: Partial<AppConfig>): Promise<AppConfig> {
    const edit: Edit = {id: ++sequence, patch: {...patch, ...(patch.game_shortcut_keys && {game_shortcut_keys: {...patch.game_shortcut_keys}}), ...(patch.custom_game_paths && {custom_game_paths: {...patch.custom_game_paths}})}};
    edits.push(edit);
    publish({pending: state.pending + 1, error: ""});
    return schedule(async () => {
      try {
        // Include backend changes made by restore/migration, never a component's stale snapshot.
        const current = await transport.load();
        const value = edits.filter(item => item.id <= edit.id).reduce((config, item) => merge(config, item.patch), current ?? editorDefaults);
        await transport.save(value);
        const saved = await transport.load();
        if (!saved) throw new Error("A configuração não foi persistida.");
        // A later successful write also commits preceding edits that previously failed.
        edits = edits.filter(item => item.id > edit.id);
        accept(saved);
        return saved;
      } catch (error) {
        publish({loaded: true, error: String(error)});
        throw error;
      } finally {publish({pending: state.pending - 1});}
    });
  }
  return {
    getSnapshot: () => state,
    subscribe: (listener: () => void) => {listeners.add(listener); return () => {listeners.delete(listener);};},
    load, refresh, update,
    retry: () => edits.length ? update({}) : refresh(),
  };
}
