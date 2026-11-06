import type {AppConfig} from "../../types/appConfig";
import type {InstallRoute} from "../../types/installer";
import {menuShortcutStorageKey} from "../../services/routeDefaults";
export interface GameShortcutChange {path: string; directory?: string; route: InstallRoute; key: string; installed: boolean}
export function createGameShortcutWriter(save: (patch: Partial<AppConfig>) => Promise<unknown>, updateInstalled: (change: GameShortcutChange) => Promise<unknown>) {
  let tail: Promise<unknown> = Promise.resolve();
  return (change: GameShortcutChange): Promise<void> => {
    // Expose the intended key immediately; INI updates still follow selection order.
    const saved = save({game_shortcut_keys: {[menuShortcutStorageKey(change.path, change.route)]: change.key}})
      .then(() => ({ok: true as const}), error => ({ok: false as const, error}));
    const task = tail.then(async () => {
      const result = await saved;
      if (!result.ok) throw result.error;
      if (change.installed) await updateInstalled(change);
    });
    tail = task.catch(() => {});
    return task;
  };
}
