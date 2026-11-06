import {invoke} from "@tauri-apps/api/core";
import {browserLanguages} from "./preference";
/** Return only locale preferences; never collect or expose the process environment. */
export async function systemLanguages(): Promise<readonly string[]> {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window && !("__PELI_UI_PREVIEW__" in window)) {
    try {
      const languages = await invoke<string[]>("system_language_preferences");
      if (Array.isArray(languages) && languages.every(value => typeof value === "string") && languages.length) return languages;
    } catch { /* An older backend can use the webview locale. */ }
  }
  return browserLanguages();
}
