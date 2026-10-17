// Run separately: bun scripts/test-matrix-presence.ts; no native/network calls.
import assert from "node:assert/strict";
import { mock } from "bun:test";
const saved = new Map<string, string>([["peligames.matrix.gameActivity", JSON.stringify({ sharing: true })]]);
Object.assign(globalThis, { localStorage: { getItem: (key: string) => saved.get(key) ?? null, setItem: (key: string, value: string) => saved.set(key, value) } });
let effects: (() => void | (() => void))[] = [];
const calls: any[] = [];
mock.module("react", () => ({ useEffect: (effect: () => void) => effects.push(effect), useSyncExternalStore: (_sub: unknown, snapshot: () => unknown) => snapshot() }));
mock.module("@tauri-apps/api/core", () => ({ invoke: async (command: string, args: unknown) => { calls.push({ command, args }); } }));
mock.module("../src/services/matrixChat", () => ({ useMatrixChat: () => ({ userId: "@me:test" }), setMatrixPresence() {} }));
const { setMatrixPresenceChoice } = await import("../src/services/matrixPresencePreferences");
const { useMatrixGameActivity } = await import("../src/services/matrixGameActivity");
const game = { id: "test", name: "My Game", library: "PeliGames", state: "running" as const, onStop: async () => {} };
for (const presence of ["online", "idle", "dnd", "invisible"] as const) {
  setMatrixPresenceChoice("@me:test", presence);
  effects = [];
  useMatrixGameActivity([game]);
  const cleanups = effects.map(effect => effect());
  await Bun.sleep(550);
  assert.equal(calls.at(-1).args.presence, presence);
  assert.equal(calls.at(-1).args.gameName, presence === "invisible" ? null : "My Game", "invisible does not publish game activity");
  cleanups.forEach(cleanup => cleanup?.());
}
console.log("Passed: all presence choices reach the backend; invisible suppresses shared game names.");
