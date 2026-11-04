import { isFrameGenerationWrapper, type AdvancedSettings } from "./advancedSettings";

/** Switch the saved launch configuration while preserving both generators' tuning. */
export function selectFrameGenerator(settings: AdvancedSettings, backend: "mako" | "afmf"): AdvancedSettings {
  if (backend === "mako") {
    return settings.afmf?.enabled ? { ...settings, afmf: { ...settings.afmf, enabled: false } } : settings;
  }
  return {
    ...settings,
    wrappers: settings.wrappers?.map(wrapper => isFrameGenerationWrapper(wrapper) && wrapper.enabled
      ? { ...wrapper, enabled: false }
      : wrapper),
  };
}
