import { describe, expect, test } from "bun:test";
import { normalizeAdvanced, toggleAdvanced, advancedError, defaultGamescope, gamescopeEnabled, validateGamescopeExtra } from "./advancedSettings";
import rules from "../config/advancedRules.json";
describe("advanced launch safety", () => {
  test("all options default off and removed options are absent", () => {
    const s = normalizeAdvanced();
    expect(Object.values(s.options).every(v => !v)).toBe(true);
    expect(s.options.wined3d).toBeUndefined();
    expect(s.launch_backend).toBe("system");
    expect(s.gamescope).toEqual(defaultGamescope);
  });
  test("legacy accidental defaults are reset once; explicit new choices survive", () => {
    const old = { options: { wayland: true }, dll_overrides: "dxgi=n,b", launch_backend: "system" as const };
    expect(normalizeAdvanced(old).options.wayland).toBe(false);
    expect(normalizeAdvanced({ ...old, schema_version: 2 }).options.wayland).toBe(true);
  });
  test("every dependency and conflicting pair is blocked with a message", () => {
    for (const [a,b] of rules.dependencies) {
      const s = normalizeAdvanced(); s.options[a] = true;
      expect(advancedError(s)).toBeDefined();
      s.options[b] = true;
    }
    for (const [a,b] of rules.conflicts) {
      const s = normalizeAdvanced(); s.options[a] = true; s.options[b] = true;
      expect(advancedError(s)).toBeDefined();
    }
  });
  test("toggle never silently changes another setting", () => {
    const s = normalizeAdvanced(); s.options.mangohud = true;
    const next = toggleAdvanced(s, "mangohud_env");
    expect(next.options.mangohud).toBe(true);
    expect(advancedError(next)).toContain("MangoHud");
    expect(s.options.mangohud_env).toBe(false);
  });
  test("Gamescope expands only enabled features and rejects unsafe extra options", () => {
    const g = { ...defaultGamescope, fps: 60 };
    expect(gamescopeEnabled(g)).toBe(false);
    g.enable_limiter = true; expect(gamescopeEnabled(g)).toBe(true);
    expect(validateGamescopeExtra('--prefer-output "Display 1"')).toBeUndefined();
    for (const value of ["-- ls", "-r 60", "--filter=nis", "--foo $(id)", "--foo 'unfinished"]) expect(validateGamescopeExtra(value)).toBeDefined();
  });
  test("Gamescope constraints and invalid DLL values are rejected", () => {
    const s = normalizeAdvanced(); s.gamescope!.enable_limiter = true; s.options.mangohud = true;
    expect(advancedError(s)).toContain("MangoHud");
    s.options.mangohud = false; s.gamescope!.fps = 1001;
    expect(advancedError(s)).toContain("FPS");
    s.gamescope!.fps = 60; s.options.dll_overrides = true; s.dll_overrides = "dxgi=n,b;echo test";
    expect(advancedError(s)).toContain("DLLs");
  });
});

test("wrapper settings preserve disabled defaults and reject incomplete quotes", () => {
  const s = normalizeAdvanced(); expect(s.wrappers).toEqual([]);
  s.wrappers = [{ program: "/tmp/wrapper with space", arguments: "--name 'two words'", enabled: false }];
  expect(advancedError(s)).toBeUndefined();
  expect(normalizeAdvanced(s).wrappers).toEqual(s.wrappers);
  s.wrappers[0].arguments = "'unfinished";
  expect(advancedError(s)).toContain("aspas");
});
