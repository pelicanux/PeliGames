import { test, expect } from "bun:test";
import { PRESETS, applyPreset } from "../src/components/preferences/mangoHudPresets";
import { mangoHudDefaults } from "../src/components/preferences/mangoHudDefaults";
for (const preset of PRESETS) {
  test(`${preset.name}: desliga VSync Vulkan e OpenGL mesmo em perfil salvo`, () => {
    const saved = {...mangoHudDefaults(), vsync: "3", gl_vsync: "2", fps_limit: 90};
    const applied = applyPreset(preset.id, saved);
    expect(applied.vsync).toBe("1");
    expect(applied.gl_vsync).toBe("0");
    expect(applied.fps_limit).toBe(90);
    expect(saved.vsync).toBe("3");
    expect(saved.gl_vsync).toBe("2");
  });
}
