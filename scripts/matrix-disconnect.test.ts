import { describe, test, expect, mock } from "bun:test";
const pending: {resolve: (value: unknown) => void; reject: (error: Error) => void}[] = [];
let stops = 0;
Object.assign(globalThis, {window: new EventTarget()});
mock.module("@tauri-apps/api/core", () => ({invoke: (command: string) => {
  if (command === "matrix_chat_rooms") return Promise.resolve({rooms: []});
  return new Promise((resolve, reject) => pending.push({resolve, reject}));
}}));
mock.module("react", () => ({useSyncExternalStore: (_subscribe: unknown, snapshot: () => unknown) => snapshot()}));
mock.module("../src/services/matrixSdk", () => ({
  startMatrixSdk: async () => {}, stopMatrixSdk: () => {stops++;}, updateMatrixSdkRooms: () => {}, retryMatrixSdk: () => {}, matrixSdkClient: async () => ({}),
}));
mock.module("../src/services/matrixDisplayNames", () => ({fetchMatrixDisplayNames: async () => ({})}));
mock.module("../src/services/matrixPresencePreferences", () => ({matrixPresenceChoice: () => "online"}));
const {refreshMatrixStatus, disconnectMatrixChat, useMatrixChat} = await import("../src/services/matrixChat");
const status = (user_id: string | null) => ({user_id, pending: false});
describe("Matrix disconnect", () => {
  test("clears the shared account, deduplicates logout and ignores a stale status reply", async () => {
    const login = refreshMatrixStatus(); pending.shift()!.resolve(status("@test:example.org")); await login;
    expect(useMatrixChat().userId).toBe("@test:example.org");
    const stale = refreshMatrixStatus(); const staleResponse = pending.shift()!;
    const logout = disconnectMatrixChat(); const response = pending.shift()!;
    expect(disconnectMatrixChat()).toBe(logout);
    await refreshMatrixStatus(); expect(pending).toHaveLength(0);
    response.resolve(true); expect(await logout).toBe(true);
    staleResponse.resolve(status("@test:example.org")); await stale;
    expect(useMatrixChat().userId).toBeNull();
    expect(useMatrixChat().rooms).toEqual({}); expect(stops).toBeGreaterThan(0);
  });
  test("offline server logout still clears local state; failure to remove credentials keeps it", async () => {
    let login = refreshMatrixStatus(); pending.shift()!.resolve(status("@test:example.org")); await login;
    let logout = disconnectMatrixChat(); pending.shift()!.reject(new Error("keyring failure"));
    await expect(logout).rejects.toThrow("keyring failure");
    expect(useMatrixChat().userId).toBe("@test:example.org");
    logout = disconnectMatrixChat(); pending.shift()!.resolve(false); expect(await logout).toBe(false);
    expect(useMatrixChat().userId).toBeNull();
    login = refreshMatrixStatus(); pending.shift()!.resolve(status(null)); await login;
    expect(useMatrixChat().userId).toBeNull();
  });
});
