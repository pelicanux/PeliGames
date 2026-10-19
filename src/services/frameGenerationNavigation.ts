export const OPEN_FRAME_GENERATION_PREFERENCES = "openFrameGenerationPreferences";
export function openFrameGenerationPreferences(backend: "mako") {
  window.dispatchEvent(new CustomEvent(OPEN_FRAME_GENERATION_PREFERENCES, {detail: backend}));
}
