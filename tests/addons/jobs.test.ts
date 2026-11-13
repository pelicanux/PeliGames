import {expect, test} from "bun:test";
import type {AddonHost} from "../../addons/_infra/ui-sdk";
import {waitForAddonJob} from "../../addons/_infra/ui-sdk/helpers/jobs";
import {connectOptiScaler} from "../../addons/optiscaler/ui/src/connection";
import {ResourceScope} from "../../src/modules/addons/lifecycle";
import {createAddonI18n} from "../../src/modules/addons/i18n";
import {addonDesign} from "../../src/modules/addons/sharedDesign";

function fixture(response: (method: string, params: unknown) => unknown) {
  const scope = new ResourceScope(() => {});
  const requests: {method: string; params: unknown}[] = [];
  const host: AddonHost = {
    ...addonDesign, apiVersion: 1, addon: {id: "optiscaler", version: "0.1.0"}, language: "en",
    lifecycle: scope, i18n: createAddonI18n("en", scope), styles: {add() {}}, setBusy() {},
    service: {async call<T>(method: string, params: unknown) {requests.push({method, params}); return response(method, params) as T}},
  };
  return {host, scope, requests};
}
test("job polling validates identity and terminal errors and cancels its pending wait", async () => {
  for (const response of [{id: 2, done: true}, {id: 1, done: true, error: "Download failed"}]) {
    const f = fixture(() => response);
    await expect(waitForAddonJob(f.host, 1)).rejects.toThrow();
    await f.scope.close();
  }
  const f = fixture(() => ({id: 1, done: false}));
  const work = waitForAddonJob(f.host, 1).catch(error => error);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  await f.scope.close();
  expect((await work).name).toBe("AbortError");
  expect(f.requests).toHaveLength(1);
});
test("OptiScaler uses only its selected service and preserves raw INI values", async () => {
  const f = fixture((method, params) => method === "game.mutate" ? {"Menu.Scale": "auto"} : {channel: "stable"});
  const connection = connectOptiScaler(f.host);
  expect(f.requests).toEqual([]);
  await connection.api.inspect("/fixture/game");
  await connection.api.update("/fixture/game","Menu.Scale","auto");
  await connection.api.install("/fixture/game","winmm.dll",{});
  await connection.api.remove("/fixture/game");
  await connection.api.removeDownload("nightly");
  expect(f.requests).toEqual([
    {method: "game.inspect", params: {directory: "/fixture/game"}},
    {method: "game.mutate", params: {directory: "/fixture/game", action: {operation: "update", key: "Menu.Scale", value: "auto"}}},
    {method: "game.mutate", params: {directory: "/fixture/game", action: {operation: "install", proxy: "winmm.dll", settings: {}}}},
    {method: "game.mutate", params: {directory: "/fixture/game", action: {operation: "remove"}}},
    {method: "runtime.remove", params: {channel: "nightly"}},
  ]);
  expect("invoke" in connection).toBe(false);
  await f.scope.close();
});
test("OptiScaler downloads report progress and stop on the service's terminal result", async () => {
  const f = fixture(method => method === "runtime.download" ? {job: 3} : {id: 3, done: true, result: null, progress: {phase: "complete", downloaded: 50, total: 50}});
  const events: unknown[] = [];
  await connectOptiScaler(f.host).api.download("nightly", progress => events.push(progress));
  expect(f.requests).toEqual([{method: "runtime.download", params: {channel: "nightly"}}, {method: "job.inspect", params: {id: 3}}]);
  expect(events).toEqual([{phase: "complete", downloaded: 50, total: 50}]);
  await f.scope.close();
});
