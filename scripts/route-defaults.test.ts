import { expect, test } from "bun:test";
import { launchOptionsFromInstallOutput } from "../src/services/routeDefaults";

test("0.0.7 uses the complete alternative line for runtime network switching", () => {
  const normal = 'WINEDLLOVERRIDES="dxgi=n,b" %command%';
  const all = 'VK_ADD_LAYER_PATH="/games/a b/dlssnr-amd/int4/layer" VK_INSTANCE_LAYERS=VK_LAYER_DLSSNR_int4 WINEDLLOVERRIDES="dxgi=n,b" %command%';
  expect(launchOptionsFromInstallOutput(`Steam launch options:  ${normal}\nTo use CC_SILVER_AARDWOLD_INT4_MIXED in game, use this line instead (all three networks can be switched):\n                 ${all}\nLogs: x`)).toBe(all);
  expect(launchOptionsFromInstallOutput(`Steam launch options:  ${all}`)).toBe(all);
});
test("older backends keep their printed command and malformed alternatives are ignored", () => {
  const line = 'WINEDLLOVERRIDES="dxgi=n,b" %command%';
  expect(launchOptionsFromInstallOutput(`Success:\nSteam launch options:  ${line}`)).toBe(line);
  expect(launchOptionsFromInstallOutput(`Steam launch options:  ${line}\nTo use CC_SILVER_AARDWOLD_INT4_MIXED in game, use this line instead (all three networks can be switched):\ninvalid`)).toBe(line);
  expect(launchOptionsFromInstallOutput("Steam launch options: invalid")).toBeNull();
});

test("Mauri RDNA3 0.0.6 preserves both int4 and NR Control layers", () => {
  const line = 'VK_ADD_LAYER_PATH="/games/a b/dlssnr-amd/int4/layer:/games/a b/dlssnr-amd/control/layer" VK_INSTANCE_LAYERS=VK_LAYER_DLSSNR_int4:VK_LAYER_DLSSNR_control WINEDLLOVERRIDES="dxgi=n,b" %command%';
  expect(launchOptionsFromInstallOutput(`Installed the optiscaler route into: /games/a b\nSteam launch options:  ${line}\nVK_ADD_LAYER_PATH and VK_INSTANCE_LAYERS in the launch options are needed by int4 mixed and the NR Control menu; copy the whole line.`)).toBe(line);
});
