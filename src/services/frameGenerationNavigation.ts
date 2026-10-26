export const OPEN_FRAME_GENERATION_PREFERENCES = "openFrameGenerationPreferences";
export function openFrameGenerationPreferences(backend: "mako" | "afmf") {
  window.dispatchEvent(new CustomEvent(OPEN_FRAME_GENERATION_PREFERENCES, {detail: backend}));
}

export function openCapturePreferences(){window.dispatchEvent(new CustomEvent(OPEN_FRAME_GENERATION_PREFERENCES,{detail:"capture"}));}
