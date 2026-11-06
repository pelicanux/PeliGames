import {expect, test} from "bun:test";
import {createGameShortcutWriter} from "../../src/modules/dlssnr/gameShortcuts";
import type {AppConfig} from "../../src/types/appConfig";
const flush = async () => {for (let i = 0; i < 12; i++) await Promise.resolve();};
test("shortcut choices enter shared persistence immediately, while INIs are updated in selection order", async () => {
  const patches: Partial<AppConfig>[] = [], started: string[] = [], completed: string[] = [];
  let release!: () => void; const gate = new Promise<void>(resolve => {release = resolve;});
  const write = createGameShortcutWriter(async patch => {patches.push(patch);}, async request => {started.push(request.path); if (request.path === "gameA") await gate; completed.push(request.path);});
  const first = write({path: "gameA", directory: "/a", route: "optiscaler", key: "F3", installed: true}); await flush();
  const second = write({path: "gameB", directory: "/b", route: "reshade", key: "F6", installed: true}); await flush();
  expect(patches.length).toBe(2); expect(started).toEqual(["gameA"]); release(); await Promise.all([first, second]);
  expect(completed).toEqual(["gameA", "gameB"]);
});
test("a failed configuration does not update its INI or poison the next request", async () => {
  const installed: string[] = []; let fail = true;
  const write = createGameShortcutWriter(async () => {if (fail) throw Error("save failed");}, async request => {installed.push(request.key);});
  await expect(write({path: "game", route: "reshade", key: "F3", installed: true})).rejects.toThrow("save failed");
  expect(installed).toEqual([]); fail = false;
  await write({path: "game", route: "reshade", key: "F4", installed: true}); expect(installed).toEqual(["F4"]);
});
test("uninstalled games save the preference without modifying game files", async () => {
  let calls = 0;
  const write = createGameShortcutWriter(async () => {}, async () => {calls++;});
  await write({path: "game", route: "reshade", key: "F3", installed: false}); expect(calls).toBe(0);
});
